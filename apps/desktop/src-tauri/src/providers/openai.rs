//! `OpenAI`-compatible provider.
//!
//! Phase 1.0.1 covers the non-streaming path (T1.0.1.20); streaming
//! is wired in T1.0.1.21 on top of the same client. The provider
//! stores the upstream URL + plaintext API key (decrypted by the
//! repository before construction) and the `reqwest` client so we
//! reuse the connection pool across calls.

use std::time::Duration;

use async_trait::async_trait;
use futures::stream::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::{Client, StatusCode};
use serde_json::json;

use super::api::{
    ApiRequest, ApiResponse, HealthStatus, Provider, ProviderError, StreamingResponse,
};

/// Default timeout for non-streaming chat-completions calls. Keep
/// short — `SwitchEngine` wants to fail-over rather than wait.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// `OpenAI` chat-completions provider.
#[derive(Clone)]
pub struct OpenAIProvider {
    id: String,
    name: String,
    base_url: String,
    api_key: String,
    priority: i32,
    cost_per_token: Option<f64>,
    client: Client,
}

impl std::fmt::Debug for OpenAIProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAIProvider")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("base_url", &self.base_url)
            .field("api_key", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl OpenAIProvider {
    /// Build a provider that calls `base_url/v1/chat/completions`.
    /// `base_url` should not include a trailing slash.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        base_url: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Result<Self, ProviderError> {
        Self::with_options(id, name, base_url, api_key, 100, None)
    }

    /// Full constructor with priority and cost. Used by
    /// `ProviderWrapper` (T1.0.2.02) when hydrating from the DB row.
    pub fn with_options(
        id: impl Into<String>,
        name: impl Into<String>,
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        priority: i32,
        cost_per_token: Option<f64>,
    ) -> Result<Self, ProviderError> {
        let client = Client::builder()
            .timeout(DEFAULT_REQUEST_TIMEOUT)
            .build()
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        Ok(Self {
            id: id.into(),
            name: name.into(),
            base_url: base_url.into(),
            api_key: api_key.into(),
            priority,
            cost_per_token,
            client,
        })
    }

    /// Compose the absolute endpoint URL for `path` (begins with `/`).
    fn endpoint(&self, path: &str) -> String {
        format!("{}{path}", self.base_url.trim_end_matches('/'))
    }

    /// Standard headers (auth + content type). Centralised so the
    /// streaming impl can reuse them in T1.0.1.21.
    fn auth_headers(&self) -> Result<HeaderMap, ProviderError> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let bearer = format!("Bearer {}", self.api_key);
        let value = HeaderValue::from_str(&bearer).map_err(|e| {
            ProviderError::BadRequest(format!("api key contains invalid bytes: {e}"))
        })?;
        headers.insert(AUTHORIZATION, value);
        Ok(headers)
    }
}

#[async_trait]
impl Provider for OpenAIProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn get_priority(&self) -> i32 {
        self.priority
    }

    fn get_cost_per_token(&self) -> Option<f64> {
        self.cost_per_token
    }

    fn get_quota_remaining(&self) -> Option<u64> {
        // OpenAI doesn't expose remaining quota in a cheap way;
        // ProviderWrapper (T1.0.2.02) will cache this from runtime
        // usage headers when available.
        None
    }

    /// Cheap probe: `GET /v1/models`. 200 ⇒ Healthy, 401/403 ⇒
    /// Down (auth issue), 5xx / network ⇒ Down (treat as out of
    /// rotation), 429 ⇒ Degraded (still up but throttled).
    async fn health_check(&self) -> Result<HealthStatus, ProviderError> {
        let url = self.endpoint("/v1/models");
        let response = self
            .client
            .get(url)
            .headers(self.auth_headers()?)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        match response.status() {
            s if s.is_success() => Ok(HealthStatus::Healthy),
            StatusCode::TOO_MANY_REQUESTS => Ok(HealthStatus::Degraded),
            _ => Ok(HealthStatus::Down),
        }
    }

    async fn send_request(&self, request: ApiRequest) -> Result<ApiResponse, ProviderError> {
        // Force non-streaming on the wire — the streaming path goes
        // through `send_stream_request`. Re-emit the body manually so
        // we don't accidentally forward `stream: true` to the upstream.
        let body = json!({
            "model": request.model,
            "messages": request.messages,
            "temperature": request.temperature,
            "max_tokens": request.max_tokens,
            "stream": false,
        });
        let response = self
            .client
            .post(self.endpoint("/v1/chat/completions"))
            .headers(self.auth_headers()?)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        let status = response.status();
        if status.is_success() {
            return response
                .json::<ApiResponse>()
                .await
                .map_err(|e| ProviderError::Decode(e.to_string()));
        }

        let body_text = response.text().await.unwrap_or_default();
        Err(map_http_error(status, body_text))
    }

    async fn send_stream_request(
        &self,
        request: ApiRequest,
    ) -> Result<StreamingResponse, ProviderError> {
        // Force `stream: true` regardless of caller input — the
        // non-streaming path is `send_request`. Splitting the wire
        // override here mirrors `send_request` which forces `false`.
        let body = json!({
            "model": request.model,
            "messages": request.messages,
            "temperature": request.temperature,
            "max_tokens": request.max_tokens,
            "stream": true,
        });
        let response = self
            .client
            .post(self.endpoint("/v1/chat/completions"))
            .headers(self.auth_headers()?)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let body_text = response.text().await.unwrap_or_default();
            return Err(map_http_error(status, body_text));
        }
        // Forward chunks verbatim; the proxy layer will repackage
        // them as SSE in T1.0.1.22 (`axum::response::Sse`).
        let upstream = response
            .bytes_stream()
            .map(|chunk| chunk.map_err(|e| ProviderError::Network(e.to_string())));
        Ok(Box::pin(upstream))
    }
}

/// Map an upstream HTTP status to the appropriate `ProviderError`.
/// Pulled into a free function so the streaming path (T1.0.1.21) can
/// reuse the same policy.
pub fn map_http_error(status: StatusCode, body: String) -> ProviderError {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => ProviderError::Unauthorized(body),
        StatusCode::BAD_REQUEST | StatusCode::NOT_FOUND | StatusCode::UNPROCESSABLE_ENTITY => {
            ProviderError::BadRequest(body)
        }
        StatusCode::TOO_MANY_REQUESTS => ProviderError::RateLimited(body),
        s if s.is_server_error() => ProviderError::Upstream {
            status: s.as_u16(),
            body,
        },
        s => ProviderError::Upstream {
            status: s.as_u16(),
            body,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_http_error_classifies_401_as_unauthorized() {
        let err = map_http_error(StatusCode::UNAUTHORIZED, "no key".into());
        assert!(matches!(err, ProviderError::Unauthorized(_)));
        assert!(!err.is_retriable());
    }

    #[test]
    fn map_http_error_classifies_429_as_rate_limited() {
        let err = map_http_error(StatusCode::TOO_MANY_REQUESTS, "rate".into());
        assert!(matches!(err, ProviderError::RateLimited(_)));
        assert!(err.is_retriable());
    }

    #[test]
    fn map_http_error_classifies_500_as_upstream_with_status() {
        let err = map_http_error(StatusCode::INTERNAL_SERVER_ERROR, "oops".into());
        match err {
            ProviderError::Upstream { status, .. } => assert_eq!(status, 500),
            other => panic!("expected Upstream, got {other:?}"),
        }
    }

    #[test]
    fn map_http_error_classifies_400_as_bad_request_not_retriable() {
        let err = map_http_error(StatusCode::BAD_REQUEST, "bad".into());
        assert!(matches!(err, ProviderError::BadRequest(_)));
        assert!(!err.is_retriable());
    }

    #[test]
    fn endpoint_handles_trailing_slash_in_base_url() {
        let p = OpenAIProvider::new("id", "n", "https://api.openai.com/", "k").expect("build");
        assert_eq!(
            p.endpoint("/v1/chat/completions"),
            "https://api.openai.com/v1/chat/completions",
        );
    }

    #[test]
    fn debug_does_not_leak_api_key() {
        let p = OpenAIProvider::new("id", "n", "https://api", "sk-secret").expect("build");
        let rendered = format!("{p:?}");
        assert!(
            !rendered.contains("sk-secret"),
            "api_key leaked: {rendered}"
        );
        assert!(rendered.contains("redacted"));
    }

    #[test]
    fn auth_headers_set_bearer_and_content_type() {
        let p = OpenAIProvider::new("id", "n", "https://api", "sk-secret").expect("build");
        let h = p.auth_headers().expect("ok");
        assert_eq!(h.get(CONTENT_TYPE).unwrap(), "application/json");
        assert_eq!(h.get(AUTHORIZATION).unwrap(), "Bearer sk-secret");
    }

    #[test]
    fn default_priority_is_100() {
        let p = OpenAIProvider::new("id", "n", "https://api", "k").expect("build");
        assert_eq!(p.get_priority(), 100);
    }

    #[test]
    fn with_options_sets_priority_and_cost() {
        let p = OpenAIProvider::with_options("id", "n", "https://api", "k", 10, Some(0.000_003))
            .expect("build");
        assert_eq!(p.get_priority(), 10);
        assert_eq!(p.get_cost_per_token(), Some(0.000_003));
    }

    #[test]
    fn quota_remaining_is_none_by_default() {
        let p = OpenAIProvider::new("id", "n", "https://api", "k").expect("build");
        assert_eq!(p.get_quota_remaining(), None);
    }

    #[test]
    fn invalid_api_key_bytes_yield_bad_request() {
        // CR / LF are illegal in header values per RFC 7230. Anyone
        // pasting a key with a stray newline gets a clean error
        // instead of a panic deep in reqwest.
        let p = OpenAIProvider::new("id", "n", "https://api", "bad\nkey").expect("build");
        let err = p.auth_headers().expect_err("must reject");
        assert!(matches!(err, ProviderError::BadRequest(_)));
    }
}
