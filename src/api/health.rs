use axum::{extract::State, Json};
use serde_json::{json, Value};

use crate::middleware::AppState;

pub async fn health_check(State(state): State<AppState>) -> Json<Value> {
    let stats = state.cache.stats();
    Json(json!({
        "status": "ok",
        "cache_enabled": state.cache.enabled,
        "cache_entries": stats.keys,
        "memory_used_bytes": stats.memory_bytes,
        "hit_rate_pct": format!("{:.2}%", stats.hit_rate),
    }))
}
