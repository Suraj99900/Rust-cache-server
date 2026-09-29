use std::time::Instant;
use bytes::Bytes;
use http::HeaderMap;

#[derive(Debug, Clone)]
pub struct CachedResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Bytes,
    pub created_at: Instant,
    pub expires_at: Instant,
    pub size_bytes: usize,
}

impl CachedResponse {
    pub fn new(
        status: u16,
        headers: &HeaderMap,
        body: Bytes,
        expires_at: Instant,
    ) -> Self {
        let created_at = Instant::now();
        
        let header_list: Vec<(String, String)> = headers
            .iter()
            .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or_default().to_string()))
            .collect();

        // Approximate size calculation
        let headers_size: usize = header_list
            .iter()
            .map(|(k, v)| k.len() + v.len())
            .sum();
        let size_bytes = body.len() + headers_size + std::mem::size_of::<Self>();

        Self {
            status,
            headers: header_list,
            body,
            created_at,
            expires_at,
            size_bytes,
        }
    }

    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }

    pub fn ttl_remaining(&self) -> Option<std::time::Duration> {
        let now = Instant::now();
        if self.expires_at > now {
            Some(self.expires_at - now)
        } else {
            None
        }
    }
}
