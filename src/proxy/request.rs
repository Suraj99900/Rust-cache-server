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

/// Extract a stable user identifier from a JWT Bearer token for cache key generation.
///
/// JWT tokens rotate on every refresh (the `exp` claim changes), so we cannot hash
/// the raw token — that would produce a different cache key on every token refresh.
/// Instead, we base64url-decode the payload segment and pull the stable `sub`,
/// `user_id`, or `userId` claim which identifies the user across refreshes.
///
/// Falls back to the full raw token string if the JWT cannot be parsed, so
/// non-JWT auth schemes (API keys, etc.) still work correctly.
pub fn extract_jwt_user_id(auth_header: &str) -> String {
    let token = auth_header
        .strip_prefix("Bearer ")
        .or_else(|| auth_header.strip_prefix("bearer "))
        .unwrap_or(auth_header);

    // JWT has three dot-separated segments: header.payload.signature
    let payload_b64 = match token.split('.').nth(1) {
        Some(p) => p,
        None => return auth_header.to_string(), // not a JWT, use as-is
    };

    // base64url → base64 standard padding
    let padded = match payload_b64.len() % 4 {
        2 => format!("{}==", payload_b64),
        3 => format!("{}=", payload_b64),
        _ => payload_b64.to_string(),
    };
    let normalized = padded.replace('-', "+").replace('_', "/");

    let decoded = match base64_decode(&normalized) {
        Some(b) => b,
        None => return auth_header.to_string(),
    };

    let payload_str = match std::str::from_utf8(&decoded) {
        Ok(s) => s,
        Err(_) => return auth_header.to_string(),
    };

    // Parse the JSON and look for common user-identity claim names
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(payload_str) {
        for claim in &["sub", "user_id", "userId", "uid", "id"] {
            if let Some(v) = json.get(claim).and_then(|v| {
                v.as_str()
                    .map(|s| s.to_string())
                    .or_else(|| v.as_u64().map(|n| n.to_string()))
            }) {
                if !v.is_empty() {
                    return v;
                }
            }
        }
    }

    // Could not find a stable claim — fall back to the raw token
    auth_header.to_string()
}

/// Minimal base64 (standard alphabet with padding) decoder — avoids adding a dep.
fn base64_decode(input: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8; 128] = b"\
        \xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\
        \xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\
        \xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\x3e\xff\xff\xff\x3f\
        \x34\x35\x36\x37\x38\x39\x3a\x3b\x3c\x3d\xff\xff\xff\xff\xff\xff\
        \xff\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\
        \x0f\x10\x11\x12\x13\x14\x15\x16\x17\x18\x19\xff\xff\xff\xff\xff\
        \xff\x1a\x1b\x1c\x1d\x1e\x1f\x20\x21\x22\x23\x24\x25\x26\x27\x28\
        \x29\x2a\x2b\x2c\x2d\x2e\x2f\x30\x31\x32\x33\xff\xff\xff\xff\xff";

    let input = input.trim_end_matches('=');
    let mut out = Vec::with_capacity(input.len() * 3 / 4 + 1);
    let bytes = input.as_bytes();
    let mut i = 0;

    while i + 3 < bytes.len() {
        let (a, b, c, d) = (bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]);
        if a > 127 || b > 127 || c > 127 || d > 127 { return None; }
        let (va, vb, vc, vd) = (TABLE[a as usize], TABLE[b as usize], TABLE[c as usize], TABLE[d as usize]);
        if va == 0xff || vb == 0xff || vc == 0xff || vd == 0xff { return None; }
        out.push((va << 2) | (vb >> 4));
        out.push((vb << 4) | (vc >> 2));
        out.push((vc << 6) | vd);
        i += 4;
    }

    let rem = bytes.len() - i;
    if rem == 2 {
        let (a, b) = (bytes[i], bytes[i + 1]);
        if a > 127 || b > 127 { return None; }
        let (va, vb) = (TABLE[a as usize], TABLE[b as usize]);
        if va == 0xff || vb == 0xff { return None; }
        out.push((va << 2) | (vb >> 4));
    } else if rem == 3 {
        let (a, b, c) = (bytes[i], bytes[i + 1], bytes[i + 2]);
        if a > 127 || b > 127 || c > 127 { return None; }
        let (va, vb, vc) = (TABLE[a as usize], TABLE[b as usize], TABLE[c as usize]);
        if va == 0xff || vb == 0xff || vc == 0xff { return None; }
        out.push((va << 2) | (vb >> 4));
        out.push((vb << 4) | (vc >> 2));
    }

    Some(out)
}
