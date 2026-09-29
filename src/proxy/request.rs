use http::HeaderMap;

const HOP_BY_HOP_HEADERS: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
    "accept-encoding",
];

/// Filter out hop-by-hop headers for proxy forwarding
pub fn filter_forward_headers(headers: &HeaderMap, client_ip: Option<&str>) -> HeaderMap {
    let mut forward_headers = HeaderMap::new();

    for (name, value) in headers.iter() {
        let name_str = name.as_str().to_lowercase();
        if !HOP_BY_HOP_HEADERS.contains(&name_str.as_str()) && name_str != "host" {
            forward_headers.insert(name.clone(), value.clone());
        }
    }

    if let Some(ip) = client_ip {
        if let Ok(val) = ip.parse() {
            forward_headers.insert("x-forwarded-for", val);
        }
    }

    forward_headers
}

/// Check if request has an authorization header or cookie (indicating private/authenticated user)
pub fn is_authenticated(headers: &HeaderMap) -> bool {
    headers.contains_key("authorization") || headers.contains_key("proxy-authorization")
}
