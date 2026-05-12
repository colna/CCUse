//! `axum` middleware that enforces protocol-scoped `sk-local-…` API
//! keys on the proxy's `/v1/*` routes.
//!
//! Two header forms accepted, in priority order: `Authorization:
//! Bearer <key>` (`OpenAI` / Anthropic newer SDKs) and `x-api-key`
//! (Anthropic legacy + Cursor). Comparison is constant-time so the
//! response timing can't reveal how many leading bytes matched.

use std::sync::{Arc, RwLock};

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;
use subtle::ConstantTimeEq;

use crate::proxy::error::{ApiError, ErrorProtocol};

/// Local proxy keys scoped by inbound API protocol. OpenAI-compatible
/// clients receive the `openai` key; Anthropic-compatible clients
/// receive the `anthropic` key.
#[derive(Clone, PartialEq, Eq)]
pub struct LocalApiKeySet {
    pub openai: String,
    pub anthropic: String,
}

impl std::fmt::Debug for LocalApiKeySet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalApiKeySet")
            .field("openai", &"<redacted>")
            .field("anthropic", &"<redacted>")
            .finish()
    }
}

impl LocalApiKeySet {
    #[must_use]
    pub fn new(openai: impl Into<String>, anthropic: impl Into<String>) -> Self {
        Self {
            openai: openai.into(),
            anthropic: anthropic.into(),
        }
    }

    fn expected_for_path(&self, path: &str) -> &str {
        match ErrorProtocol::for_path(path) {
            ErrorProtocol::Anthropic => &self.anthropic,
            ErrorProtocol::OpenAi => &self.openai,
        }
    }
}

impl From<String> for LocalApiKeySet {
    fn from(value: String) -> Self {
        Self::new(value.clone(), value)
    }
}

impl From<&str> for LocalApiKeySet {
    fn from(value: &str) -> Self {
        Self::new(value, value)
    }
}

/// Live, mutably-shared expected keys. `RwLock` because regeneration
/// happens off the hot request path; reads are cheap and uncontended.
pub type KeyStore = Arc<RwLock<LocalApiKeySet>>;

/// Build a fresh [`KeyStore`] seeded with one legacy key or a
/// protocol-scoped key set.
#[must_use]
pub fn key_store(initial: impl Into<LocalApiKeySet>) -> KeyStore {
    Arc::new(RwLock::new(initial.into()))
}

/// Pull the presented key from request headers. `Authorization:
/// Bearer …` wins over `x-api-key` when both are set — matches what
/// real clients ship and avoids ambiguity if a client sends both.
fn extract_presented_key(headers: &axum::http::HeaderMap) -> Option<String> {
    if let Some(value) = headers.get(axum::http::header::AUTHORIZATION) {
        if let Ok(raw) = value.to_str() {
            // Case-insensitive scheme per RFC 7235; tolerate extra
            // whitespace after the scheme.
            let trimmed = raw.trim();
            if trimmed.len() >= 7 && trimmed[..7].eq_ignore_ascii_case("Bearer ") {
                let token = trimmed[7..].trim_start();
                if !token.is_empty() {
                    return Some(token.to_owned());
                }
            }
        }
    }
    if let Some(value) = headers.get("x-api-key") {
        if let Ok(raw) = value.to_str() {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_owned());
            }
        }
    }
    None
}

/// Reject the request unless it presents the current `sk-local-…`
/// token. Mounted via [`axum::middleware::from_fn_with_state`].
pub async fn require_local_api_key(
    State(store): State<KeyStore>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let protocol = ErrorProtocol::for_path(request.uri().path());
    let Some(presented) = extract_presented_key(request.headers()) else {
        return Err(ApiError::unauthorized(
            "missing api key: send `Authorization: Bearer sk-local-...` or `x-api-key`",
        )
        .with_protocol(protocol));
    };
    // Snapshot once; release the read lock before the constant-time
    // compare so a slow path can't backpressure regenerate_api_key.
    let expected = {
        let guard = store.read().map_err(|_| {
            ApiError::internal("auth keystore lock poisoned").with_protocol(protocol)
        })?;
        guard.expected_for_path(request.uri().path()).to_owned()
    };
    if presented.as_bytes().ct_eq(expected.as_bytes()).into() {
        Ok(next.run(request).await)
    } else {
        Err(ApiError::unauthorized("invalid api key").with_protocol(protocol))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};

    fn headers_with(name: &'static str, value: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(name, HeaderValue::from_str(value).unwrap());
        h
    }

    #[test]
    fn empty_headers_yield_no_key() {
        assert!(extract_presented_key(&HeaderMap::new()).is_none());
    }

    #[test]
    fn bearer_authorization_extracts_token() {
        let h = headers_with("authorization", "Bearer sk-local-abcd");
        assert_eq!(extract_presented_key(&h).as_deref(), Some("sk-local-abcd"),);
    }

    #[test]
    fn bearer_scheme_is_case_insensitive() {
        let h = headers_with("authorization", "bEaReR  sk-local-abcd");
        assert_eq!(extract_presented_key(&h).as_deref(), Some("sk-local-abcd"),);
    }

    #[test]
    fn x_api_key_falls_back_when_authorization_missing() {
        let h = headers_with("x-api-key", "sk-local-xyz");
        assert_eq!(extract_presented_key(&h).as_deref(), Some("sk-local-xyz"));
    }

    #[test]
    fn authorization_takes_priority_over_x_api_key() {
        let mut h = headers_with("authorization", "Bearer sk-local-correct");
        h.insert("x-api-key", HeaderValue::from_static("sk-local-stale"));
        assert_eq!(
            extract_presented_key(&h).as_deref(),
            Some("sk-local-correct"),
        );
    }

    #[test]
    fn whitespace_only_x_api_key_is_treated_as_missing() {
        let h = headers_with("x-api-key", "   ");
        assert!(extract_presented_key(&h).is_none());
    }

    #[test]
    fn non_bearer_authorization_falls_through() {
        // Basic auth / token scheme not recognised; we fall through
        // to x-api-key. Without one, no key.
        let h = headers_with("authorization", "Basic Zm9vOmJhcg==");
        assert!(extract_presented_key(&h).is_none());
    }

    #[test]
    fn protocol_key_set_uses_openai_key_for_openai_surfaces() {
        let keys = LocalApiKeySet::new("sk-local-openai", "sk-local-anthropic");

        assert_eq!(
            keys.expected_for_path("/v1/chat/completions"),
            "sk-local-openai"
        );
        assert_eq!(keys.expected_for_path("/v1/models"), "sk-local-openai");
    }

    #[test]
    fn protocol_key_set_uses_anthropic_key_for_messages() {
        let keys = LocalApiKeySet::new("sk-local-openai", "sk-local-anthropic");

        assert_eq!(keys.expected_for_path("/v1/messages"), "sk-local-anthropic");
    }

    #[test]
    fn legacy_single_key_populates_both_protocols() {
        let keys = LocalApiKeySet::from("sk-local-legacy");

        assert_eq!(keys.openai, "sk-local-legacy");
        assert_eq!(keys.anthropic, "sk-local-legacy");
    }
}
