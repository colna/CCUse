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
use serde::Deserialize;
use serde_json::{json, Value};

use super::api::{
    ApiModel, ApiRequest, ApiResponse, HealthStatus, Provider, ProviderError, StreamingResponse,
};
use super::default_models::{
    model_candidates, should_try_next_default_model, OPENAI_DEFAULT_MODELS,
};
use super::error_format::format_reqwest_error;
use super::model;
use super::stream_check::{check_provider_with_default_config, health_status_from_stream_result};

/// Default timeout for provider HTTP calls.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(600);

/// `OpenAI` chat-completions provider.
#[derive(Clone)]
pub struct OpenAIProvider {
    id: String,
    name: String,
    base_url: String,
    api_key: String,
    priority: i32,
    cost_per_token: Option<f64>,
    default_models: Vec<String>,
    client: Client,
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    #[serde(default)]
    data: Vec<ApiModel>,
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
        Self::with_default_models_and_options(
            id,
            name,
            base_url,
            api_key,
            priority,
            cost_per_token,
            OPENAI_DEFAULT_MODELS
                .iter()
                .map(|model| (*model).to_owned())
                .collect(),
        )
    }

    #[doc(hidden)]
    pub fn with_options_and_timeout(
        id: impl Into<String>,
        name: impl Into<String>,
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        priority: i32,
        cost_per_token: Option<f64>,
        timeout: Duration,
    ) -> Result<Self, ProviderError> {
        let mut provider =
            Self::with_options(id, name, base_url, api_key, priority, cost_per_token)?;
        provider.client = Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| ProviderError::Network(format_reqwest_error(e)))?;
        Ok(provider)
    }

    pub fn with_default_models(
        id: impl Into<String>,
        name: impl Into<String>,
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        default_models: Vec<String>,
    ) -> Result<Self, ProviderError> {
        Self::with_default_models_and_options(
            id,
            name,
            base_url,
            api_key,
            100,
            None,
            default_models,
        )
    }

    fn with_default_models_and_options(
        id: impl Into<String>,
        name: impl Into<String>,
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        priority: i32,
        cost_per_token: Option<f64>,
        default_models: Vec<String>,
    ) -> Result<Self, ProviderError> {
        let client = Client::builder()
            .timeout(DEFAULT_REQUEST_TIMEOUT)
            .build()
            .map_err(|e| ProviderError::Network(format_reqwest_error(e)))?;
        Ok(Self {
            id: id.into(),
            name: name.into(),
            base_url: base_url.into(),
            api_key: api_key.into(),
            priority,
            cost_per_token,
            default_models,
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

    /// Stream probe matching cc-switch's model-test semantics.
    async fn health_check(&self) -> Result<HealthStatus, ProviderError> {
        let provider = self.stream_check_provider();
        let result = check_provider_with_default_config(&provider, &self.api_key)
            .await
            .map_err(|err| ProviderError::Network(err.to_string()))?;
        Ok(health_status_from_stream_result(&result))
    }

    async fn list_models(&self) -> Result<Vec<ApiModel>, ProviderError> {
        let response = self
            .client
            .get(self.endpoint("/v1/models"))
            .headers(self.auth_headers()?)
            .send()
            .await
            .map_err(|e| ProviderError::Network(format_reqwest_error(e)))?;

        let status = response.status();
        if status.is_success() {
            return response
                .json::<ModelsResponse>()
                .await
                .map(|body| body.data)
                .map_err(|e| ProviderError::Decode(format_reqwest_error(e)));
        }

        let body_text = response.text().await.unwrap_or_default();
        Err(map_http_error(status, body_text))
    }

    async fn send_request(&self, request: ApiRequest) -> Result<ApiResponse, ProviderError> {
        // Force non-streaming on the wire — the streaming path goes
        // through `send_stream_request`.
        let candidates = model_candidates(&request.model, &self.default_models);
        let mut last_error = None;
        for model in candidates {
            let body = chat_completion_body(request_with_model(&request, &model), false)?;
            let response = self
                .client
                .post(self.endpoint("/v1/chat/completions"))
                .headers(self.auth_headers()?)
                .json(&body)
                .send()
                .await
                .map_err(|e| ProviderError::Network(format_reqwest_error(e)))?;

            let status = response.status();
            if status.is_success() {
                return response
                    .json::<ApiResponse>()
                    .await
                    .map_err(|e| ProviderError::Decode(format_reqwest_error(e)));
            }

            let body_text = response.text().await.unwrap_or_default();
            let error = map_http_error(status, body_text);
            if should_try_next_default_model(&error) {
                last_error = Some(error);
                continue;
            }
            return Err(error);
        }
        Err(last_error
            .unwrap_or_else(|| ProviderError::BadRequest("no default model candidates".into())))
    }

    async fn send_stream_request(
        &self,
        request: ApiRequest,
    ) -> Result<StreamingResponse, ProviderError> {
        // Force `stream: true` regardless of caller input — the
        // non-streaming path is `send_request`. Splitting the wire
        // override here mirrors `send_request` which forces `false`.
        let candidates = model_candidates(&request.model, &self.default_models);
        let mut last_error = None;
        for model in candidates {
            let body = chat_completion_body(request_with_model(&request, &model), true)?;
            let response = self
                .client
                .post(self.endpoint("/v1/chat/completions"))
                .headers(self.auth_headers()?)
                .json(&body)
                .send()
                .await
                .map_err(|e| ProviderError::Network(format_reqwest_error(e)))?;

            let status = response.status();
            if !status.is_success() {
                let body_text = response.text().await.unwrap_or_default();
                let error = map_http_error(status, body_text);
                if should_try_next_default_model(&error) {
                    last_error = Some(error);
                    continue;
                }
                return Err(error);
            }
            let upstream = response
                .bytes_stream()
                .map(|chunk| chunk.map_err(|e| ProviderError::Network(format_reqwest_error(e))));
            return Ok(Box::pin(upstream));
        }
        Err(last_error
            .unwrap_or_else(|| ProviderError::BadRequest("no default model candidates".into())))
    }
}

impl OpenAIProvider {
    fn stream_check_provider(&self) -> model::Provider {
        model::Provider {
            id: self.id.clone(),
            name: self.name.clone(),
            kind: model::ProviderKind::Openai,
            base_url: self.base_url.clone(),
            priority: self.priority,
            enabled: true,
            monthly_quota: None,
            rate_limit_rpm: None,
            cost_per_1k_tokens: self.cost_per_token.map(|cost| cost * 1000.0),
            created_at: String::new(),
            updated_at: String::new(),
        }
    }
}

fn chat_completion_body(mut request: ApiRequest, stream: bool) -> Result<Value, ProviderError> {
    request.stream = stream;
    let tools = std::mem::take(&mut request.tools);
    let mut body = serde_json::to_value(request).map_err(|e| {
        ProviderError::BadRequest(format!("failed to serialize provider request: {e}"))
    })?;
    add_tools_to_body(&mut body, &tools);
    Ok(body)
}

fn request_with_model(request: &ApiRequest, model: &str) -> ApiRequest {
    let mut mapped = request.clone();
    model.clone_into(&mut mapped.model);
    mapped
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

fn add_tools_to_body(body: &mut serde_json::Value, tools: &[super::api::ApiToolDefinition]) {
    if tools.is_empty() {
        return;
    }
    let values: Vec<_> = tools
        .iter()
        .map(|tool| {
            let mut function = json!({
                "name": tool.name,
                "parameters": tool.parameters,
            });
            if let Some(description) = &tool.description {
                function["description"] = json!(description);
            }
            json!({
                "type": "function",
                "function": function,
            })
        })
        .collect();
    body["tools"] = json!(values);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::api::{ApiToolDefinition, ChatMessage};

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
    fn default_request_timeout_matches_long_provider_deadline() {
        assert_eq!(DEFAULT_REQUEST_TIMEOUT, Duration::from_secs(600));
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

    #[test]
    fn chat_completion_body_omits_none_fields_and_wraps_tools() {
        let body = chat_completion_body(
            ApiRequest {
                model: "gpt-5.4".into(),
                messages: vec![ChatMessage {
                    role: "user".into(),
                    content: "ping".into(),
                    tool_call_id: None,
                    tool_calls: vec![],
                }],
                temperature: None,
                max_tokens: None,
                stream: false,
                tools: vec![ApiToolDefinition {
                    name: "get_weather".into(),
                    description: Some("Get weather".into()),
                    parameters: json!({"type": "object"}),
                }],
            },
            true,
        )
        .expect("serialize request body");

        assert_eq!(body["stream"], true);
        assert_eq!(body["model"], "gpt-5.4");
        assert!(body.get("temperature").is_none());
        assert!(body.get("max_tokens").is_none());
        assert_eq!(body["tools"][0]["type"], "function");
        assert_eq!(body["tools"][0]["function"]["name"], "get_weather");
    }
}
