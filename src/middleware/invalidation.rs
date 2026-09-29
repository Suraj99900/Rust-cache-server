use http::StatusCode;
use tracing::info;
use crate::cache::CacheEngine;

pub struct CacheInvalidator;

impl CacheInvalidator {
    /// Invalidate cache for a given path upon successful mutating request
    pub fn handle_mutation_success(
        cache: &CacheEngine,
        path: &str,
        status: StatusCode,
        request_id: &str,
    ) {
        if status.is_success() {
            let count = cache.invalidate_path(path);
            info!(
                request_id = request_id,
                path = path,
                status = status.as_u16(),
                invalidated_entries = count,
                "Cache invalidated after mutation"
            );
        }
    }
}
