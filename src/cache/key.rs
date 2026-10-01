use std::fmt;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey(String);

impl CacheKey {
    /// Create a normalized cache key from HTTP method, URI path, optional query string,
    /// an optional request body (for POST caching), and an optional auth token (for
    /// per-user isolation). Both body and auth are hashed so raw values are never stored.
    pub fn new(method: &str, path: &str, query: Option<&str>, body: Option<&[u8]>, auth: Option<&str>) -> Self {
        let normalized_query = query.map(Self::normalize_query).filter(|q| !q.is_empty());

        let body_suffix = body
            .filter(|b| !b.is_empty())
            .map(|b| {
                let mut hasher = DefaultHasher::new();
                b.hash(&mut hasher);
                format!(":body={:x}", hasher.finish())
            })
            .unwrap_or_default();

        let auth_suffix = auth
            .filter(|a| !a.is_empty())
            .map(|a| {
                let mut hasher = DefaultHasher::new();
                a.hash(&mut hasher);
                format!(":auth={:x}", hasher.finish())
            })
            .unwrap_or_default();

        let key_str = match normalized_query {
            Some(ref q) => format!("{}:{}:?{}{}{}", method.to_uppercase(), path, q, body_suffix, auth_suffix),
            None => format!("{}:{}{}{}", method.to_uppercase(), path, body_suffix, auth_suffix),
        };
        CacheKey(key_str)
    }

    /// Create a key from raw string
    pub fn from_raw(raw: String) -> Self {
        CacheKey(raw)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Normalize query string by sorting key-value pairs alphabetically
    fn normalize_query(query: &str) -> String {
        let mut pairs: Vec<(&str, &str)> = query
            .split('&')
            .filter(|s| !s.is_empty())
            .map(|param| {
                if let Some((k, v)) = param.split_once('=') {
                    (k, v)
                } else {
                    (param, "")
                }
            })
            .collect();

        pairs.sort_by(|a, b| a.0.cmp(b.0).then_with(|| a.1.cmp(b.1)));

        let mut normalized = String::with_capacity(query.len());
        for (i, (k, v)) in pairs.into_iter().enumerate() {
            if i > 0 {
                normalized.push('&');
            }
            normalized.push_str(k);
            if !v.is_empty() {
                normalized.push('=');
                normalized.push_str(v);
            }
        }
        normalized
    }
}

impl fmt::Display for CacheKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_normalization() {
        let key1 = CacheKey::new("GET", "/api/products", Some("page=1&limit=20"), None, None);
        let key2 = CacheKey::new("get", "/api/products", Some("limit=20&page=1"), None, None);
        assert_eq!(key1, key2);
        assert_eq!(key1.as_str(), "GET:/api/products:?limit=20&page=1");
    }

    #[test]
    fn test_cache_key_no_query() {
        let key = CacheKey::new("GET", "/api/users/100", None, None, None);
        assert_eq!(key.as_str(), "GET:/api/users/100");
    }

    #[test]
    fn test_post_cache_key_differs_by_body() {
        let key1 = CacheKey::new("POST", "/api/search", None, Some(b"{\"q\":\"foo\"}"), None);
        let key2 = CacheKey::new("POST", "/api/search", None, Some(b"{\"q\":\"bar\"}"), None);
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_post_cache_key_same_body() {
        let key1 = CacheKey::new("POST", "/api/search", None, Some(b"{\"q\":\"foo\"}"), None);
        let key2 = CacheKey::new("POST", "/api/search", None, Some(b"{\"q\":\"foo\"}"), None);
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_auth_token_isolates_cache_entries() {
        let key1 = CacheKey::new("POST", "/api/v1/participant/get/policies", None, Some(b"{}"), Some("Bearer token-user-1"));
        let key2 = CacheKey::new("POST", "/api/v1/participant/get/policies", None, Some(b"{}"), Some("Bearer token-user-2"));
        assert_ne!(key1, key2, "Different auth tokens must produce different cache keys");
    }

    #[test]
    fn test_same_auth_token_hits_same_cache_entry() {
        let key1 = CacheKey::new("POST", "/api/v1/participant/get/policies", None, Some(b"{}"), Some("Bearer token-user-1"));
        let key2 = CacheKey::new("POST", "/api/v1/participant/get/policies", None, Some(b"{}"), Some("Bearer token-user-1"));
        assert_eq!(key1, key2, "Same auth token + same body must produce the same cache key");
    }
}
