use std::time::Instant;

use axum::{
    extract::{Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use http_body_util::BodyExt;
use tracing::{info, instrument};
use uuid::Uuid;

use crate::{
    cache::{CacheEngine, CacheKey, CachedResponse},
    errors::GatewayError,
    metrics::MetricsRegistry,
    middleware::{
        invalidation::CacheInvalidator,
        policy::{CacheAction, CachePolicy},
    },
    proxy::{
        build_hit_response, build_miss_response, is_response_cacheable, BackendForwarder,
    },
};

#[derive(Clone)]
pub struct AppState {
    pub cache: CacheEngine,
    pub forwarder: BackendForwarder,
    pub metrics: MetricsRegistry,
    pub admin_api_key: Option<String>,
}

/// The universal reverse proxy handler for all incoming client requests
#[axum::debug_handler(state = AppState)]
#[instrument(skip(state, request), fields(request_id))]
pub async fn proxy_handler(
    State(state): State<AppState>,
    request: Request,
) -> Response {
    match proxy_handler_inner(state, request).await {
        Ok(resp) => resp,
        Err(err) => err.into_response(),
    }
}

async fn proxy_handler_inner(
    state: AppState,
    request: Request,
) -> Result<Response, GatewayError> {
    let start = Instant::now();

    let (parts, body) = request.into_parts();
    let method = parts.method;
    let uri = parts.uri;
    let headers = parts.headers;

    // Generate or propagate request ID
    let request_id = headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let path = uri.path().to_string();
    let query = uri.query().map(|q| q.to_string());
    let path_and_query = uri
        .path_and_query()
        .map(|pq| pq.as_str().to_string())
        .unwrap_or_else(|| path.clone());

    // Read request body with a safety limit (10MB)
    let body_bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(_) => return Err(GatewayError::PayloadTooLarge),
    };

    let action = CachePolicy::determine_action(&method, &headers);

    match action {
        CacheAction::Cacheable => {
            let cache_key = CacheKey::new(method.as_str(), &path, query.as_deref());

            // 1. Check cache
            if let Some(cached) = state.cache.get(&cache_key) {
                let duration = start.elapsed();
                state.metrics.cache_hits.inc();
                state
                    .metrics
                    .http_requests_total
                    .with_label_values(&[method.as_str(), "HIT"])
                    .inc();
                state
                    .metrics
                    .http_request_duration_seconds
                    .with_label_values(&["HIT"])
                    .observe(duration.as_secs_f64());

                info!(
                    request_id = %request_id,
                    method = method.as_str(),
                    path = %path,
                    cache_status = "HIT",
                    duration_ms = duration.as_millis(),
                    "Cache HIT served"
                );

                return Ok(build_hit_response(&cached, Some(&request_id)));
            }

            // 2. Cache MISS - Single-flight coalescing
            state.metrics.cache_misses.inc();
            let forwarder = state.forwarder.clone();
            let cache_engine = state.cache.clone();
            let method_clone = method.clone();
            let path_and_query_clone = path_and_query.clone();
            let headers_clone = headers.clone();
            let body_clone = body_bytes.clone();
            let req_id_clone = request_id.clone();
            let path_clone = path.clone();
            let key_clone = cache_key.clone();

            let flight_result = state
                .cache
                .single_flight
                .execute(cache_key.as_str(), || async move {
                    let backend_res = forwarder
                        .forward(
                            method_clone,
                            &path_and_query_clone,
                            &headers_clone,
                            body_clone,
                            None,
                            &req_id_clone,
                        )
                        .await
                        .map_err(|e| e.to_string())?;

                    let is_cacheable = is_response_cacheable(backend_res.status, &backend_res.headers);
                    if is_cacheable {
                        cache_engine.set(
                            &key_clone,
                            &path_clone,
                            backend_res.status.as_u16(),
                            &backend_res.headers,
                            backend_res.body.clone(),
                            None,
                        );
                    }

                    let expires_at = std::time::Instant::now()
                        + cache_engine.ttl_manager.get_ttl(&path_clone);
                    Ok(CachedResponse::new(
                        backend_res.status.as_u16(),
                        &backend_res.headers,
                        backend_res.body,
                        expires_at,
                    ))
                })
                .await;

            let duration = start.elapsed();
            state
                .metrics
                .http_requests_total
                .with_label_values(&[method.as_str(), "MISS"])
                .inc();
            state
                .metrics
                .http_request_duration_seconds
                .with_label_values(&["MISS"])
                .observe(duration.as_secs_f64());

            match flight_result {
                Ok(cached_res) => {
                    info!(
                        request_id = %request_id,
                        method = method.as_str(),
                        path = %path,
                        status = cached_res.status,
                        cache_status = "MISS",
                        duration_ms = duration.as_millis(),
                        "Cache MISS resolved and response returned"
                    );
                    Ok(build_miss_response(
                        StatusCode::from_u16(cached_res.status).unwrap_or(StatusCode::OK),
                        &headers,
                        cached_res.body,
                        Some(&request_id),
                    ))
                }
                Err(err) => Err(GatewayError::Internal(err)),
            }
        }

        CacheAction::Invalidate => {
            let backend_res = state
                .forwarder
                .forward(
                    method.clone(),
                    &path_and_query,
                    &headers,
                    body_bytes,
                    None,
                    &request_id,
                )
                .await?;

            CacheInvalidator::handle_mutation_success(
                &state.cache,
                &path,
                backend_res.status,
                &request_id,
            );

            let duration = start.elapsed();
            state
                .metrics
                .http_requests_total
                .with_label_values(&[method.as_str(), "BYPASS"])
                .inc();
            state
                .metrics
                .http_request_duration_seconds
                .with_label_values(&["BYPASS"])
                .observe(duration.as_secs_f64());

            Ok(build_miss_response(
                backend_res.status,
                &backend_res.headers,
                backend_res.body,
                Some(&request_id),
            ))
        }

        CacheAction::Bypass => {
            let backend_res = state
                .forwarder
                .forward(
                    method.clone(),
                    &path_and_query,
                    &headers,
                    body_bytes,
                    None,
                    &request_id,
                )
                .await?;

            let duration = start.elapsed();
            state
                .metrics
                .http_requests_total
                .with_label_values(&[method.as_str(), "BYPASS"])
                .inc();
            state
                .metrics
                .http_request_duration_seconds
                .with_label_values(&["BYPASS"])
                .observe(duration.as_secs_f64());

            Ok(build_miss_response(
                backend_res.status,
                &backend_res.headers,
                backend_res.body,
                Some(&request_id),
            ))
        }
    }
}
