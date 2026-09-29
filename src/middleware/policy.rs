use http::{HeaderMap, Method};
use crate::proxy::request::is_authenticated;

#[derive(Debug, PartialEq, Eq)]
pub enum CacheAction {
    /// Attempt to serve from cache or populate cache on miss
    Cacheable,
    /// Forward request without caching
    Bypass,
    /// Forward request and invalidate matching cache paths on success
    Invalidate,
}

pub struct CachePolicy;

impl CachePolicy {
    pub fn determine_action(method: &Method, headers: &HeaderMap) -> CacheAction {
        // Authenticated requests bypass public caching by default to prevent data leakage
        if is_authenticated(headers) {
            return match *method {
                Method::PUT | Method::PATCH | Method::DELETE => CacheAction::Invalidate,
                _ => CacheAction::Bypass,
            };
        }

        match *method {
            Method::GET | Method::HEAD => CacheAction::Cacheable,
            Method::POST => CacheAction::Bypass,
            Method::PUT | Method::PATCH | Method::DELETE => CacheAction::Invalidate,
            _ => CacheAction::Bypass,
        }
    }
}
