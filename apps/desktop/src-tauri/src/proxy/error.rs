//! Unified API error type for proxy handlers.
//!
//! Handlers that return `Result<T, ApiError>` get automatic
//! conversion to `(StatusCode, Json<Value>)` via [`IntoResponse`],
//! producing an `OpenAI`-shaped error envelope:
//!
//! ```json
//! { "error": { "type": "...", "message": "..." } }
//! ```

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

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
    /// Catch-all internal failure.
    Internal,
}

impl ApiErrorKind {
    /// HTTP status mapped to this error kind.
    #[must_use]
    pub const fn status(self) -> StatusCode {
        match self {
            Self::ProvidersNotConfigured => StatusCode::SERVICE_UNAVAILABLE,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::BadRequest => StatusCode::BAD_REQUEST,
            Self::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Stable wire string for `error.type`.
    #[must_use]
    pub const fn type_str(self) -> &'static str {
        match self {
            Self::ProvidersNotConfigured => "providers_not_configured",
            Self::Unauthorized => "unauthorized",
            Self::BadRequest => "bad_request",
            Self::TooManyRequests => "rate_limit_exceeded",
            Self::Internal => "internal_error",
        }
    }
}

/// Error returned by proxy HTTP handlers.
#[derive(Debug, thiserror::Error)]
#[error("{kind:?}: {message}")]
pub struct ApiError {
    pub kind: ApiErrorKind,
    pub message: String,
}

impl ApiError {
    pub fn new(kind: ApiErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
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
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.kind.status();
        let body = Json(json!({
            "error": {
                "type": self.kind.type_str(),
                "message": self.message,
            }
        }));
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
            ApiErrorKind::Internal.status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn type_str_is_stable_wire_id() {
        // These strings are part of the public wire contract; if they
        // change clients break, so the test pins them deliberately.
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
        assert_eq!(ApiErrorKind::Internal.type_str(), "internal_error");
    }

    #[test]
    fn display_includes_kind_and_message() {
        let err = ApiError::unauthorized("missing api key");
        let rendered = format!("{err}");
        assert!(rendered.contains("Unauthorized"));
        assert!(rendered.contains("missing api key"));
    }
}
