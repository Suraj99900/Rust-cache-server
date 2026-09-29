use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use crate::{cache::CacheKey, middleware::AppState};

fn is_admin_authorized(headers: &HeaderMap, configured_key: Option<&str>) -> bool {
    if let Some(expected_key) = configured_key {
        if let Some(auth_val) = headers.get("x-admin-key") {
            if let Ok(val) = auth_val.to_str() {
                return val == expected_key;
            }
        }
        false
    } else {
        true
    }
}

pub async fn admin_stats(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !is_admin_authorized(&headers, state.admin_api_key.as_deref()) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let stats = state.cache.stats();
    Ok(Json(json!({
        "keys": stats.keys,
        "hits": stats.hits,
        "misses": stats.misses,
        "sets": stats.sets,
        "deletes": stats.deletes,
        "evictions": stats.evictions,
        "expired": stats.expired,
        "memory_bytes": stats.memory_bytes,
        "hit_rate_pct": format!("{:.2}%", stats.hit_rate),
        "shard_count": stats.shard_count,
    })))
}

pub async fn admin_purge_all(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !is_admin_authorized(&headers, state.admin_api_key.as_deref()) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    state.cache.clear_all();
    Ok(Json(json!({
        "message": "All cache entries successfully purged",
    })))
}

pub async fn admin_purge_key(
    State(state): State<AppState>,
    Path(key): Path<String>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !is_admin_authorized(&headers, state.admin_api_key.as_deref()) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let cache_key = CacheKey::from_raw(key.clone());
    let deleted = state.cache.delete(&cache_key);

    if deleted {
        Ok(Json(json!({
            "key": key,
            "deleted": true,
            "message": "Cache key deleted"
        })))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

pub async fn prometheus_metrics(State(state): State<AppState>) -> Response {
    let output = state.metrics.render_prometheus();
    (
        StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4")],
        output,
    )
        .into_response()
}
