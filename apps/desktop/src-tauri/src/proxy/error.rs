//! Unified API error type for proxy handlers.
//!
//! Handlers that return `Result<T, ApiError>` get automatic
//! conversion to `(StatusCode, Json<Value>)` via [`IntoResponse`],
//! producing an error envelope that matches the inbound protocol:
//!
//! ```json
//! { "error": { "type": "...", "message": "..." } }
//! ```
//!
//! ```json
//! { "type": "error", "error": { "type": "...", "message": "..." } }
//! ```

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

use crate::converter::ConvertError;
use crate::providers::api::ProviderError;

/// Wire error envelope to use for a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorProtocol {
    OpenAi,
    Anthropic,
}

impl ErrorProtocol {
    /// Infer the error protocol from the request path.
    #[must_use]
    pub fn for_path(path: &str) -> Self {
        if path == "/v1/messages" || path.starts_with("/v1/messages/") {
            Self::Anthropic
        } else {
            Self::OpenAi
        }
    }
}

/// Kind of error surfaced over the wire.
///
/// `type_str` is mapped 1:1 to `OpenAI`'s `error.type` field so
/// generic clients that already parse `OpenAI` errors keep working.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiErrorKind {
    /// Provider dispatch is not yet wired (Phase 1.0.1 stub).
    ProvidersNotConfigured,
    /// Local API key is missing or invalid (T1.0.1.13).
    Unauthorized,
    /// Client sent a syntactically or semantically wrong request.
    BadRequest,
    /// Rate limit reached on the proxy itself (distinct from upstream 429).
    TooManyRequests,
    /// All enabled providers failed to serve the request.
    UpstreamError,
    /// Non-streaming handler exceeded its deadline.
    Timeout,
    /// No enabled providers available to handle the request.
    NoProvider,
    /// Catch-all internal failure.
    Internal,
}

impl ApiErrorKind {
    /// HTTP status mapped to this error kind.
    #[must_use]
    pub const fn status(self) -> StatusCode {
        match self {
            Self::ProvidersNotConfigured | Self::NoProvider => StatusCode::SERVICE_UNAVAILABLE,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::BadRequest => StatusCode::BAD_REQUEST,
            Self::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
            Self::UpstreamError => StatusCode::BAD_GATEWAY,
            Self::Timeout => StatusCode::GATEWAY_TIMEOUT,
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Stable wire string for `error.type`.
    #[must_use]
    pub const fn type_str(self) -> &'static str {
        match self {
            Self::ProvidersNotConfigured => "providers_not_configured",
            Self::NoProvider => "no_provider_available",
            Self::Unauthorized => "unauthorized",
            Self::BadRequest => "bad_request",
            Self::TooManyRequests => "rate_limit_exceeded",
            Self::UpstreamError => "upstream_error",
            Self::Timeout => "request_timeout",
            Self::Internal => "internal_error",
        }
    }

    /// Stable wire string for Anthropic's `error.type` field.
    #[must_use]
    pub const fn anthropic_type_str(self) -> &'static str {
        match self {
            Self::Unauthorized => "authentication_error",
            Self::BadRequest => "invalid_request_error",
            Self::TooManyRequests => "rate_limit_error",
            Self::ProvidersNotConfigured
            | Self::NoProvider
            | Self::UpstreamError
            | Self::Timeout
            | Self::Internal => "api_error",
        }
    }
}

/// Error returned by proxy HTTP handlers.
#[derive(Debug, thiserror::Error)]
#[error("{kind:?}: {message}")]
pub struct ApiError {
    pub kind: ApiErrorKind,
    pub message: String,
    protocol: ErrorProtocol,
}

impl ApiError {
    pub fn new(kind: ApiErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            protocol: ErrorProtocol::OpenAi,
        }
    }

    #[must_use]
    pub fn with_protocol(mut self, protocol: ErrorProtocol) -> Self {
        self.protocol = protocol;
        self
    }

    #[must_use]
    pub fn anthropic(self) -> Self {
        self.with_protocol(ErrorProtocol::Anthropic)
    }

    pub fn providers_not_configured() -> Self {
        Self::new(
            ApiErrorKind::ProvidersNotConfigured,
            "Provider dispatch is not wired yet. \
             Configure providers in CCUse settings (Phase 1.0.1 stub).",
        )
    }

    pub fn unauthorized(reason: impl Into<String>) -> Self {
        Self::new(ApiErrorKind::Unauthorized, reason)
    }

    pub fn bad_request(reason: impl Into<String>) -> Self {
        Self::new(ApiErrorKind::BadRequest, reason)
    }

    pub fn internal(reason: impl Into<String>) -> Self {
        Self::new(ApiErrorKind::Internal, reason)
    }

    pub fn timeout(reason: impl Into<String>) -> Self {
        Self::new(ApiErrorKind::Timeout, reason)
    }
}

impl From<ProviderError> for ApiError {
    fn from(err: ProviderError) -> Self {
        match err {
            ProviderError::Network(msg) => Self::new(ApiErrorKind::UpstreamError, msg),
            ProviderError::Upstream { status, body } => Self::new(
                ApiErrorKind::UpstreamError,
                format!("upstream returned {status}: {body}"),
            ),
            ProviderError::Unauthorized(msg) => Self::new(ApiErrorKind::Unauthorized, msg),
            ProviderError::RateLimited(msg) => Self::new(ApiErrorKind::TooManyRequests, msg),
            ProviderError::BadRequest(msg) => Self::new(ApiErrorKind::BadRequest, msg),
            ProviderError::Decode(msg) => Self::new(ApiErrorKind::Internal, msg),
        }
    }
}

impl From<ConvertError> for ApiError {
    fn from(err: ConvertError) -> Self {
        Self::new(ApiErrorKind::BadRequest, err.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.kind.status();
        let body = match self.protocol {
            ErrorProtocol::OpenAi => Json(json!({
                "error": {
                    "type": self.kind.type_str(),
                    "message": self.message,
                }
            })),
            ErrorProtocol::Anthropic => Json(json!({
                "type": "error",
                "error": {
                    "type": self.kind.anthropic_type_str(),
                    "message": self.message,
                }
            })),
        };
        (status, body).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_kind_maps_to_expected_status() {
        assert_eq!(
            ApiErrorKind::ProvidersNotConfigured.status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            ApiErrorKind::Unauthorized.status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(ApiErrorKind::BadRequest.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            ApiErrorKind::TooManyRequests.status(),
            StatusCode::TOO_MANY_REQUESTS
        );
        assert_eq!(
            ApiErrorKind::UpstreamError.status(),
            StatusCode::BAD_GATEWAY
        );
        assert_eq!(ApiErrorKind::Timeout.status(), StatusCode::GATEWAY_TIMEOUT);
        assert_eq!(
            ApiErrorKind::NoProvider.status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            ApiErrorKind::Internal.status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn type_str_is_stable_wire_id() {
        assert_eq!(
            ApiErrorKind::ProvidersNotConfigured.type_str(),
            "providers_not_configured"
        );
        assert_eq!(ApiErrorKind::Unauthorized.type_str(), "unauthorized");
        assert_eq!(ApiErrorKind::BadRequest.type_str(), "bad_request");
        assert_eq!(
            ApiErrorKind::TooManyRequests.type_str(),
            "rate_limit_exceeded"
        );
        assert_eq!(ApiErrorKind::UpstreamError.type_str(), "upstream_error");
        assert_eq!(ApiErrorKind::Timeout.type_str(), "request_timeout");
        assert_eq!(ApiErrorKind::NoProvider.type_str(), "no_provider_available");
        assert_eq!(ApiErrorKind::Internal.type_str(), "internal_error");
    }

    #[test]
    fn anthropic_type_str_maps_to_anthropic_error_ids() {
        assert_eq!(
            ApiErrorKind::Unauthorized.anthropic_type_str(),
            "authentication_error"
        );
        assert_eq!(
            ApiErrorKind::BadRequest.anthropic_type_str(),
            "invalid_request_error"
        );
        assert_eq!(
            ApiErrorKind::TooManyRequests.anthropic_type_str(),
            "rate_limit_error"
        );
        assert_eq!(ApiErrorKind::NoProvider.anthropic_type_str(), "api_error");
        assert_eq!(ApiErrorKind::Timeout.anthropic_type_str(), "api_error");
        assert_eq!(
            ApiErrorKind::UpstreamError.anthropic_type_str(),
            "api_error"
        );
    }

    #[test]
    fn error_protocol_is_inferred_from_path() {
        assert_eq!(
            ErrorProtocol::for_path("/v1/messages"),
            ErrorProtocol::Anthropic
        );
        assert_eq!(
            ErrorProtocol::for_path("/v1/messages/foo"),
            ErrorProtocol::Anthropic
        );
        assert_eq!(
            ErrorProtocol::for_path("/v1/chat/completions"),
            ErrorProtocol::OpenAi
        );
    }

    #[tokio::test]
    async fn into_response_uses_openai_envelope_by_default() {
        let response = ApiError::bad_request("missing model").into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body bytes");
        let body: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        assert_eq!(body["error"]["type"], "bad_request");
        assert!(body.get("type").is_none());
    }

    #[tokio::test]
    async fn into_response_uses_anthropic_envelope_when_marked() {
        let response = ApiError::bad_request("missing model")
            .anthropic()
            .into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body bytes");
        let body: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        assert_eq!(body["type"], "error");
        assert_eq!(body["error"]["type"], "invalid_request_error");
    }

    #[test]
    fn display_includes_kind_and_message() {
        let err = ApiError::unauthorized("missing api key");
        let rendered = format!("{err}");
        assert!(rendered.contains("Unauthorized"));
        assert!(rendered.contains("missing api key"));
    }

    #[test]
    fn from_provider_error_network_maps_to_upstream() {
        let err: ApiError = ProviderError::Network("ETIMEDOUT".into()).into();
        assert_eq!(err.kind, ApiErrorKind::UpstreamError);
        assert!(err.message.contains("ETIMEDOUT"));
    }

    #[test]
    fn from_provider_error_upstream_maps_to_bad_gateway() {
        let err: ApiError = ProviderError::Upstream {
            status: 502,
            body: "bad gateway".into(),
        }
        .into();
        assert_eq!(err.kind, ApiErrorKind::UpstreamError);
        assert!(err.message.contains("502"));
    }

    #[test]
    fn from_provider_error_unauthorized_preserved() {
        let err: ApiError = ProviderError::Unauthorized("invalid key".into()).into();
        assert_eq!(err.kind, ApiErrorKind::Unauthorized);
    }

    #[test]
    fn from_provider_error_rate_limited_maps_to_429() {
        let err: ApiError = ProviderError::RateLimited("slow down".into()).into();
        assert_eq!(err.kind, ApiErrorKind::TooManyRequests);
    }

    #[test]
    fn from_provider_error_bad_request_preserved() {
        let err: ApiError = ProviderError::BadRequest("unknown model".into()).into();
        assert_eq!(err.kind, ApiErrorKind::BadRequest);
    }

    #[test]
    fn from_provider_error_decode_maps_to_internal() {
        let err: ApiError = ProviderError::Decode("unexpected eof".into()).into();
        assert_eq!(err.kind, ApiErrorKind::Internal);
    }

    #[test]
    fn from_convert_error_maps_to_bad_request() {
        let err: ApiError = ConvertError::MissingField("model".into()).into();
        assert_eq!(err.kind, ApiErrorKind::BadRequest);
        assert!(err.message.contains("model"));
    }
}
