use std::time::{Duration, Instant};

use bytes::Bytes;
use http::{HeaderMap, Method, StatusCode};
use reqwest::Client;
use tracing::{error, info, warn};

use crate::{errors::GatewayError, metrics::MetricsRegistry, proxy::request::filter_forward_headers};

#[derive(Clone)]
pub struct BackendForwarder {
    client: Client,
    backend_url: String,
    timeout: Duration,
    metrics: MetricsRegistry,
}

pub struct BackendResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
    pub duration: Duration,
}

impl BackendForwarder {
    pub fn new(
        backend_url: String,
        timeout_seconds: u64,
        metrics: MetricsRegistry,
    ) -> Self {
        let timeout = Duration::from_secs(timeout_seconds);
        let client = Client::builder()
            .timeout(timeout)
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(100)
            .build()
            .expect("Failed to build HTTP client for backend forwarder");

        Self {
            client,
            backend_url: backend_url.trim_end_matches('/').to_string(),
            timeout,
            metrics,
        }
    }

    pub async fn forward(
        &self,
        method: Method,
        path_and_query: &str,
        headers: &HeaderMap,
        body: Bytes,
        client_ip: Option<&str>,
        request_id: &str,
    ) -> Result<BackendResponse, GatewayError> {
        let target_url = format!("{}{}", self.backend_url, path_and_query);
        let forward_headers = filter_forward_headers(headers, client_ip);

        let req_method = method.clone();
        let start = Instant::now();

        let req_builder = self
            .client
            .request(method, &target_url)
            .headers(forward_headers)
            .header("x-request-id", request_id)
            .body(body);

        let response = match req_builder.send().await {
            Ok(resp) => resp,
            Err(err) => {
                let duration = start.elapsed();
                self.metrics
                    .backend_duration_seconds
                    .with_label_values(&[req_method.as_str()])
                    .observe(duration.as_secs_f64());

                if err.is_timeout() {
                    warn!(
                        request_id = request_id,
                        target_url = %target_url,
                        duration_ms = duration.as_millis(),
                        "Backend request timed out"
                    );
                    self.metrics
                        .backend_errors
                        .with_label_values(&["timeout"])
                        .inc();
                    return Err(GatewayError::BackendTimeout);
                }

                error!(
                    request_id = request_id,
                    target_url = %target_url,
                    duration_ms = duration.as_millis(),
                    error = %err,
                    "Backend unreachable"
                );
                self.metrics
                    .backend_errors
                    .with_label_values(&["unreachable"])
                    .inc();
                return Err(GatewayError::BackendUnreachable(err.to_string()));
            }
        };

        let duration = start.elapsed();
        let status = StatusCode::from_u16(response.status().as_u16())
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

        self.metrics
            .backend_duration_seconds
            .with_label_values(&[req_method.as_str()])
            .observe(duration.as_secs_f64());

        self.metrics
            .backend_requests
            .with_label_values(&[req_method.as_str(), status.as_str()])
            .inc();

        let resp_headers = response.headers().clone();
        let resp_body = match response.bytes().await {
            Ok(b) => b,
            Err(err) => {
                error!(
                    request_id = request_id,
                    error = %err,
                    "Failed to read backend response body"
                );
                return Err(GatewayError::Internal("Failed to read backend body".to_string()));
            }
        };

        info!(
            request_id = request_id,
            method = req_method.as_str(),
            url = %target_url,
            status = status.as_u16(),
            backend_duration_ms = duration.as_millis(),
            body_bytes = resp_body.len(),
            "Backend request completed"
        );

        Ok(BackendResponse {
            status,
            headers: resp_headers,
            body: resp_body,
            duration,
        })
    }
}
