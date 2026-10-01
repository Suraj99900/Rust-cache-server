use http::{HeaderMap, HeaderValue, Method};
use rust_cache_server::cache::CacheKey;
use rust_cache_server::middleware::{CacheAction, CachePolicy};
use rust_cache_server::proxy::request::{filter_forward_headers, is_authenticated};

#[test]
fn test_query_param_normalization() {
    let key1 = CacheKey::new("GET", "/api/products", Some("b=2&a=1&c=3"), None, None);
    let key2 = CacheKey::new("GET", "/api/products", Some("c=3&b=2&a=1"), None, None);
    assert_eq!(key1, key2);
    assert_eq!(key1.as_str(), "GET:/api/products:?a=1&b=2&c=3");
}

#[test]
fn test_method_cache_policy() {
    let headers = HeaderMap::new();

    assert_eq!(
        CachePolicy::determine_action(&Method::GET, &headers),
        CacheAction::Cacheable
    );
    assert_eq!(
        CachePolicy::determine_action(&Method::HEAD, &headers),
        CacheAction::Cacheable
    );
    assert_eq!(
        CachePolicy::determine_action(&Method::POST, &headers),
        CacheAction::Cacheable
    );
    assert_eq!(
        CachePolicy::determine_action(&Method::PUT, &headers),
        CacheAction::Invalidate
    );
    assert_eq!(
        CachePolicy::determine_action(&Method::PATCH, &headers),
        CacheAction::Invalidate
    );
    assert_eq!(
        CachePolicy::determine_action(&Method::DELETE, &headers),
        CacheAction::Invalidate
    );
}

#[test]
fn test_authenticated_requests_bypass_cache() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", HeaderValue::from_static("Bearer secret_token"));

    assert!(is_authenticated(&headers));
    assert_eq!(
        CachePolicy::determine_action(&Method::GET, &headers),
        CacheAction::Bypass
    );
}

#[test]
fn test_hop_by_hop_header_stripping() {
    let mut headers = HeaderMap::new();
    headers.insert("connection", HeaderValue::from_static("keep-alive"));
    headers.insert("upgrade", HeaderValue::from_static("websocket"));
    headers.insert("content-type", HeaderValue::from_static("application/json"));
    headers.insert("accept", HeaderValue::from_static("application/json"));

    let forwarded = filter_forward_headers(&headers, Some("127.0.0.1"));

    assert!(!forwarded.contains_key("connection"));
    assert!(!forwarded.contains_key("upgrade"));
    assert!(forwarded.contains_key("content-type"));
    assert!(forwarded.contains_key("accept"));
    assert_eq!(
        forwarded.get("x-forwarded-for").unwrap(),
        "127.0.0.1"
    );
}
