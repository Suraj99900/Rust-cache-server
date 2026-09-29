use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug)]
pub enum GatewayError {
    BackendTimeout,
    BackendUnreachable(String),
    BackendError(StatusCode, String),
    PayloadTooLarge,
    Internal(String),
}

impl std::fmt::Display for GatewayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GatewayError::BackendTimeout => write!(f, "Gateway Timeout: backend did not respond in time"),
            GatewayError::BackendUnreachable(err) => write!(f, "Bad Gateway: backend unreachable: {}", err),
            GatewayError::BackendError(status, err) => write!(f, "Backend error ({}): {}", status, err),
            GatewayError::PayloadTooLarge => write!(f, "Payload Too Large: request body exceeds configured limit"),
            GatewayError::Internal(err) => write!(f, "Internal error: {}", err),
        }
    }
}

impl std::error::Error for GatewayError {}

impl IntoResponse for GatewayError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            GatewayError::BackendTimeout => (StatusCode::GATEWAY_TIMEOUT, "504 Gateway Timeout"),
            GatewayError::BackendUnreachable(ref msg) => (StatusCode::BAD_GATEWAY, msg.as_str()),
            GatewayError::BackendError(status, ref msg) => (status, msg.as_str()),
            GatewayError::PayloadTooLarge => (StatusCode::PAYLOAD_TOO_LARGE, "413 Payload Too Large"),
            GatewayError::Internal(ref msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.as_str()),
        };

        let body = Json(json!({
            "error": message,
            "status": status.as_u16(),
        }));

        (status, body).into_response()
    }
}
