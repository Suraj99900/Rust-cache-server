use axum::{
    body::Body,
    http::{HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use http::{HeaderMap, Method};

use crate::cache::CachedResponse;

const STRIPPED_RESPONSE_HEADERS: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
    "content-encoding",
    "content-length",
];

/// Determine if a backend response is cacheable based on status code and headers
pub fn is_response_cacheable(status: StatusCode, headers: &HeaderMap, method: &Method) -> bool {
    // Only cache successful responses or 404s (if negative caching enabled)
    let status_cacheable = status.is_success() || status == StatusCode::NOT_FOUND;
    if !status_cacheable {
        return false;
    }

    // POST responses are intentionally cached per-user (auth token is hashed into the
    // cache key), so Cache-Control: private / no-store from the backend does not apply —
    // each user's response is fully isolated.
    if *method == Method::POST {
        return true;
    }

    if let Some(cache_control) = headers.get("cache-control") {
        if let Ok(cc_str) = cache_control.to_str() {
            let cc_lower = cc_str.to_lowercase();
            if cc_lower.contains("no-store") || cc_lower.contains("private") {
                return false;
            }
        }
    }

    true
}

/// Build an Axum Response from a CachedResponse (Cache HIT)
pub fn build_hit_response(cached: &CachedResponse, request_id: Option<&str>) -> Response {
    let mut builder = Response::builder()
        .status(StatusCode::from_u16(cached.status).unwrap_or(StatusCode::OK))
        .header("x-cache", "HIT");

    if let Some(req_id) = request_id {
        builder = builder.header("x-request-id", req_id);
    }

    if let Some(remaining) = cached.ttl_remaining() {
        builder = builder.header("x-cache-ttl-remaining", remaining.as_secs().to_string());
    }

    for (name, val) in &cached.headers {
        let name_lower = name.to_lowercase();
        if !STRIPPED_RESPONSE_HEADERS.contains(&name_lower.as_str()) && name_lower != "x-cache" {
            if let (Ok(h_name), Ok(h_val)) = (
                HeaderName::from_bytes(name.as_bytes()),
                HeaderValue::from_str(val),
            ) {
                builder = builder.header(h_name, h_val);
            }
        }
    }

    builder
        .body(Body::from(cached.body.clone()))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// Build an Axum Response for a forward backend result (Cache MISS)
pub fn build_miss_response(
    status: StatusCode,
    headers: &HeaderMap,
    body: Bytes,
    request_id: Option<&str>,
) -> Response {
    let mut builder = Response::builder()
        .status(status)
        .header("x-cache", "MISS");

    if let Some(req_id) = request_id {
        builder = builder.header("x-request-id", req_id);
    }

    for (name, val) in headers.iter() {
        let name_str = name.as_str().to_lowercase();
        if !STRIPPED_RESPONSE_HEADERS.contains(&name_str.as_str()) && name_str != "x-cache" {
            builder = builder.header(name.clone(), val.clone());
        }
    }

    builder
        .body(Body::from(body))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}
