use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey(String);

impl CacheKey {
    /// Create a normalized cache key from HTTP method, URI path, and optional query string
    pub fn new(method: &str, path: &str, query: Option<&str>) -> Self {
        let normalized_query = query.map(Self::normalize_query).filter(|q| !q.is_empty());
        let key_str = match normalized_query {
            Some(ref q) => format!("{}:{}:?{}", method.to_uppercase(), path, q),
            None => format!("{}:{}", method.to_uppercase(), path),
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
        let key1 = CacheKey::new("GET", "/api/products", Some("page=1&limit=20"));
        let key2 = CacheKey::new("get", "/api/products", Some("limit=20&page=1"));
        assert_eq!(key1, key2);
        assert_eq!(key1.as_str(), "GET:/api/products:?limit=20&page=1");
    }

    #[test]
    fn test_cache_key_no_query() {
        let key = CacheKey::new("GET", "/api/users/100", None);
        assert_eq!(key.as_str(), "GET:/api/users/100");
    }
}
