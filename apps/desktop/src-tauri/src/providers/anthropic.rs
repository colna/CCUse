//! Anthropic Messages-compatible provider.
//!
//! This runtime is used for official Anthropic and Claude-compatible
//! relay providers. It speaks `/v1/messages` upstream, then maps the
//! response back into the provider-layer OpenAI-shaped [`ApiResponse`]
//! so the existing switch/proxy pipeline can stay protocol-neutral.

use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::StreamExt;
use reqwest::header::{
    HeaderMap, HeaderValue, ACCEPT, ACCEPT_ENCODING, AUTHORIZATION, CONTENT_TYPE, USER_AGENT,
};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::VecDeque;

use crate::converter::sse::parse_sse_frames;
use crate::converter::{
    AnthropicConverter, ContentPart, FinishReason, FormatConverter, OpenAIConverter, Role,
    ToolCall, ToolDefinition, ToolResult, UnifiedMessage, UnifiedRequest, UnifiedResponse,
};

use super::api::{
    ApiChoice, ApiModel, ApiRequest, ApiResponse, ApiToolCall, ApiToolCallFunction, ApiUsage,
    ChatMessage, HealthStatus, Provider, ProviderError, StreamingResponse,
};
use super::openai::{map_http_error, DEFAULT_REQUEST_TIMEOUT};

const HEALTH_CHECK_MODELS: &[&str] = &[
    "claude-haiku-4-5-20251001",
    "claude-opus-4-6",
    "claude-3-5-sonnet-20241022",
];
const HEALTH_CHECK_MAX_TOKENS: u32 = 1;
const HEALTH_CHECK_PROMPT: &str = "Who are you?";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const CLAUDE_CODE_BETA: &str = "claude-code-20250219,interleaved-thinking-2025-05-14";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnthropicAuthMode {
    ApiKey,
    Bearer,
}

impl AnthropicAuthMode {
    fn for_base_url(base_url: &str) -> Self {
        if base_url.contains("api.anthropic.com") {
            Self::ApiKey
        } else {
            Self::Bearer
        }
    }
}

enum StreamHealthProbe {
    Healthy,
    Status(StatusCode),
}

/// Provider for Anthropic Messages-compatible endpoints.
#[derive(Clone)]
pub struct AnthropicProvider {
    id: String,
    name: String,
    base_url: String,
    api_key: String,
    priority: i32,
    cost_per_token: Option<f64>,
    auth_mode: AnthropicAuthMode,
    client: Client,
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    #[serde(default)]
    data: Vec<ApiModel>,
}

impl std::fmt::Debug for AnthropicProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnthropicProvider")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("base_url", &self.base_url)
            .field("auth_mode", &self.auth_mode)
            .field("api_key", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl AnthropicProvider {
    /// Build a provider that calls `base_url/v1/messages`.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        base_url: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Result<Self, ProviderError> {
        Self::with_options(id, name, base_url, api_key, 100, None)
    }

    /// Full constructor with priority and cost.
    pub fn with_options(
        id: impl Into<String>,
        name: impl Into<String>,
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        priority: i32,
        cost_per_token: Option<f64>,
    ) -> Result<Self, ProviderError> {
        let base_url = base_url.into();
        let client = Client::builder()
            .timeout(DEFAULT_REQUEST_TIMEOUT)
            .build()
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        Ok(Self {
            id: id.into(),
            name: name.into(),
            auth_mode: AnthropicAuthMode::for_base_url(&base_url),
            base_url,
            api_key: api_key.into(),
            priority,
            cost_per_token,
            client,
        })
    }

    fn endpoint(&self, path: &str) -> String {
        let mut url = format!("{}{}", self.base_url.trim_end_matches('/'), path);
        while url.contains("/v1/v1") {
            url = url.replace("/v1/v1", "/v1");
        }
        url
    }

    fn auth_headers(&self) -> Result<HeaderMap, ProviderError> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            reqwest::header::HeaderName::from_static("anthropic-version"),
            HeaderValue::from_static(ANTHROPIC_VERSION),
        );
        headers.insert(
            reqwest::header::HeaderName::from_static("anthropic-beta"),
            HeaderValue::from_static(CLAUDE_CODE_BETA),
        );
        headers.insert(
            reqwest::header::HeaderName::from_static("anthropic-dangerous-direct-browser-access"),
            HeaderValue::from_static("true"),
        );
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static("claude-cli/2.1.2 (external, cli)"),
        );
        headers.insert(
            reqwest::header::HeaderName::from_static("x-app"),
            HeaderValue::from_static("cli"),
        );
        headers.insert(
            reqwest::header::HeaderName::from_static("x-stainless-lang"),
            HeaderValue::from_static("js"),
        );
        headers.insert(
            reqwest::header::HeaderName::from_static("x-stainless-package-version"),
            HeaderValue::from_static("0.70.0"),
        );
        headers.insert(
            reqwest::header::HeaderName::from_static("x-stainless-runtime"),
            HeaderValue::from_static("node"),
        );
        headers.insert(
            reqwest::header::HeaderName::from_static("x-stainless-runtime-version"),
            HeaderValue::from_static("v22.20.0"),
        );

        match self.auth_mode {
            AnthropicAuthMode::ApiKey => {
                let value = HeaderValue::from_str(&self.api_key).map_err(|e| {
                    ProviderError::BadRequest(format!("api key contains invalid bytes: {e}"))
                })?;
                headers.insert(reqwest::header::HeaderName::from_static("x-api-key"), value);
            }
            AnthropicAuthMode::Bearer => {
                let bearer = format!("Bearer {}", self.api_key);
                let value = HeaderValue::from_str(&bearer).map_err(|e| {
                    ProviderError::BadRequest(format!("api key contains invalid bytes: {e}"))
                })?;
                headers.insert(AUTHORIZATION, value);
            }
        }

        Ok(headers)
    }

    fn stream_headers(&self) -> Result<HeaderMap, ProviderError> {
        let mut headers = self.auth_headers()?;
        headers.insert(ACCEPT, HeaderValue::from_static("text/event-stream"));
        headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("identity"));
        Ok(headers)
    }

    async fn stream_health_probe(&self) -> Result<HealthStatus, ProviderError> {
        let mut saw_model_rejection = false;
        for model in HEALTH_CHECK_MODELS {
            match self.send_stream_health_probe(model).await? {
                StreamHealthProbe::Healthy => return Ok(HealthStatus::Healthy),
                StreamHealthProbe::Status(
                    StatusCode::BAD_REQUEST
                    | StatusCode::NOT_FOUND
                    | StatusCode::UNPROCESSABLE_ENTITY,
                ) => saw_model_rejection = true,
                StreamHealthProbe::Status(
                    status @ (StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN),
                ) => return Ok(status_to_health(status)),
                StreamHealthProbe::Status(StatusCode::TOO_MANY_REQUESTS) => {
                    return Ok(HealthStatus::Degraded);
                }
                StreamHealthProbe::Status(status) if status.is_server_error() => {
                    return Ok(HealthStatus::Degraded);
                }
                StreamHealthProbe::Status(_) => return Ok(HealthStatus::Degraded),
            }
        }

        Ok(if saw_model_rejection {
            HealthStatus::Degraded
        } else {
            HealthStatus::Down
        })
    }

    async fn send_stream_health_probe(
        &self,
        model: &str,
    ) -> Result<StreamHealthProbe, ProviderError> {
        let body = json!({
            "model": model,
            "messages": [{"role": "user", "content": HEALTH_CHECK_PROMPT}],
            "temperature": 0.0,
            "max_tokens": HEALTH_CHECK_MAX_TOKENS,
            "stream": true,
        });
        let response = self
            .client
            .post(self.endpoint("/v1/messages"))
            .headers(self.stream_headers()?)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            return Ok(StreamHealthProbe::Status(status));
        }

        let mut stream = response.bytes_stream();
        match stream.next().await {
            Some(Ok(_)) => Ok(StreamHealthProbe::Healthy),
            Some(Err(err)) => Err(ProviderError::Network(err.to_string())),
            None => Ok(StreamHealthProbe::Status(StatusCode::NO_CONTENT)),
        }
    }

    fn body_for_request(request: &ApiRequest, stream: bool) -> Result<Value, ProviderError> {
        let mut unified = api_request_to_unified(request);
        unified.stream = stream;
        AnthropicConverter::new()
            .unified_to_request(&unified)
            .map_err(|e| ProviderError::Decode(e.to_string()))
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
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
        None
    }

    async fn health_check(&self) -> Result<HealthStatus, ProviderError> {
        self.stream_health_probe().await
    }

    async fn list_models(&self) -> Result<Vec<ApiModel>, ProviderError> {
        let response = self
            .client
            .get(self.endpoint("/v1/models"))
            .headers(self.auth_headers()?)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        let status = response.status();
        if status.is_success() {
            return response
                .json::<ModelsResponse>()
                .await
                .map(|body| body.data)
                .map_err(|e| ProviderError::Decode(e.to_string()));
        }

        let body_text = response.text().await.unwrap_or_default();
        Err(map_http_error(status, body_text))
    }

    async fn send_request(&self, request: ApiRequest) -> Result<ApiResponse, ProviderError> {
        let body = Self::body_for_request(&request, false)?;
        let response = self
            .client
            .post(self.endpoint("/v1/messages"))
            .headers(self.auth_headers()?)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        let status = response.status();
        if status.is_success() {
            let body = response
                .json::<Value>()
                .await
                .map_err(|e| ProviderError::Decode(e.to_string()))?;
            let unified = AnthropicConverter::new()
                .response_to_unified(&body)
                .map_err(|e| ProviderError::Decode(e.to_string()))?;
            return Ok(unified_response_to_api_response(&unified));
        }

        let body_text = response.text().await.unwrap_or_default();
        Err(map_http_error(status, body_text))
    }

    async fn send_stream_request(
        &self,
        request: ApiRequest,
    ) -> Result<StreamingResponse, ProviderError> {
        let body = Self::body_for_request(&request, true)?;
        let response = self
            .client
            .post(self.endpoint("/v1/messages"))
            .headers(self.stream_headers()?)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let body_text = response.text().await.unwrap_or_default();
            return Err(map_http_error(status, body_text));
        }

        Ok(anthropic_sse_to_openai_sse(Box::pin(
            response
                .bytes_stream()
                .map(|chunk| chunk.map_err(|e| ProviderError::Network(e.to_string()))),
        )))
    }
}

fn api_request_to_unified(req: &ApiRequest) -> UnifiedRequest {
    UnifiedRequest {
        model: req.model.clone(),
        messages: req.messages.iter().map(chat_message_to_unified).collect(),
        temperature: req.temperature,
        max_tokens: req.max_tokens,
        top_p: None,
        stop: None,
        stream: req.stream,
        tools: req
            .tools
            .iter()
            .map(|tool| ToolDefinition {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
            })
            .collect(),
    }
}

fn chat_message_to_unified(message: &ChatMessage) -> UnifiedMessage {
    if let Some(tool_call_id) = &message.tool_call_id {
        return UnifiedMessage {
            role: Role::Tool,
            content: vec![ContentPart::ToolResult(ToolResult {
                tool_call_id: tool_call_id.clone(),
                output: message.content.clone(),
            })],
            name: None,
        };
    }

    let mut content = Vec::new();
    if !message.content.is_empty() {
        content.push(ContentPart::Text {
            text: message.content.clone(),
        });
    }
    content.extend(message.tool_calls.iter().map(|call| {
        ContentPart::ToolCall(ToolCall {
            id: call.id.clone(),
            name: call.function.name.clone(),
            arguments: call.function.arguments.clone(),
        })
    }));

    UnifiedMessage {
        role: parse_role(&message.role),
        content,
        name: None,
    }
}

fn unified_response_to_api_response(resp: &UnifiedResponse) -> ApiResponse {
    ApiResponse {
        id: resp.id.clone(),
        model: resp.model.clone(),
        choices: resp
            .choices
            .iter()
            .map(|choice| ApiChoice {
                index: choice.index,
                message: unified_message_to_chat(&choice.message),
                finish_reason: choice
                    .finish_reason
                    .map(finish_reason_to_api)
                    .map(str::to_owned),
            })
            .collect(),
        usage: resp.usage.as_ref().map(|usage| ApiUsage {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
        }),
    }
}

fn unified_message_to_chat(message: &UnifiedMessage) -> ChatMessage {
    let mut content = String::new();
    let mut tool_calls = Vec::new();
    let mut tool_call_id = None;

    for part in &message.content {
        match part {
            ContentPart::Text { text } => content.push_str(text),
            ContentPart::ToolCall(call) => tool_calls.push(ApiToolCall {
                id: call.id.clone(),
                kind: "function".to_owned(),
                function: ApiToolCallFunction {
                    name: call.name.clone(),
                    arguments: call.arguments.clone(),
                },
            }),
            ContentPart::ToolResult(result) => {
                tool_call_id = Some(result.tool_call_id.clone());
                content.push_str(&result.output);
            }
            ContentPart::ImageUrl { .. } => {}
        }
    }

    ChatMessage {
        role: role_to_api(message.role).to_owned(),
        content,
        tool_call_id,
        tool_calls,
    }
}

fn parse_role(role: &str) -> Role {
    match role {
        "system" => Role::System,
        "assistant" => Role::Assistant,
        "tool" => Role::Tool,
        _ => Role::User,
    }
}

fn role_to_api(role: Role) -> &'static str {
    match role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::Tool => "tool",
    }
}

fn finish_reason_to_api(reason: FinishReason) -> &'static str {
    match reason {
        FinishReason::Stop => "stop",
        FinishReason::Length => "length",
        FinishReason::ToolCalls => "tool_calls",
        FinishReason::ContentFilter => "content_filter",
    }
}

fn status_to_health(status: StatusCode) -> HealthStatus {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => HealthStatus::Down,
        StatusCode::TOO_MANY_REQUESTS => HealthStatus::Degraded,
        s if s.is_server_error() => HealthStatus::Degraded,
        s if s.is_success() => HealthStatus::Healthy,
        _ => HealthStatus::Degraded,
    }
}

fn anthropic_sse_to_openai_sse(upstream: StreamingResponse) -> StreamingResponse {
    Box::pin(futures::stream::unfold(
        AnthropicToOpenAiStreamState::new(upstream),
        |mut state| async move {
            loop {
                if let Some(item) = state.pending.pop_front() {
                    return Some((item, state));
                }

                match state.upstream.next().await {
                    Some(Ok(bytes)) => match std::str::from_utf8(&bytes) {
                        Ok(text) => {
                            state.buffer.push_str(text);
                            state.drain_complete_frames();
                        }
                        Err(err) => {
                            return Some((Err(ProviderError::Decode(err.to_string())), state));
                        }
                    },
                    Some(Err(err)) => return Some((Err(err), state)),
                    None => {
                        state.flush_trailing_frame();
                        return state.pending.pop_front().map(|item| (item, state));
                    }
                }
            }
        },
    ))
}

struct AnthropicToOpenAiStreamState {
    upstream: StreamingResponse,
    buffer: String,
    pending: VecDeque<Result<Bytes, ProviderError>>,
    anthropic: AnthropicConverter,
    openai: OpenAIConverter,
    done_emitted: bool,
}

impl AnthropicToOpenAiStreamState {
    fn new(upstream: StreamingResponse) -> Self {
        Self {
            upstream,
            buffer: String::new(),
            pending: VecDeque::new(),
            anthropic: AnthropicConverter::new(),
            openai: OpenAIConverter::new(),
            done_emitted: false,
        }
    }

    fn drain_complete_frames(&mut self) {
        while let Some(end) = self.buffer.find("\n\n") {
            let raw = self.buffer[..end + 2].to_owned();
            self.buffer.drain(..end + 2);
            self.push_frame_results(&raw);
        }
    }

    fn flush_trailing_frame(&mut self) {
        if self.buffer.trim().is_empty() {
            self.buffer.clear();
            return;
        }
        let raw = std::mem::take(&mut self.buffer);
        self.push_frame_results(&raw);
    }

    fn push_frame_results(&mut self, raw: &str) {
        for frame in parse_sse_frames(raw) {
            if frame.data == "[DONE]" || frame.event.as_deref() == Some("message_stop") {
                self.push_done();
                continue;
            }

            let chunk = match self.anthropic.parse_stream_chunk(&frame.data) {
                Ok(Some(chunk)) => chunk,
                Ok(None) => continue,
                Err(err) => {
                    self.pending
                        .push_back(Err(ProviderError::Decode(err.to_string())));
                    continue;
                }
            };

            match self.openai.encode_stream_chunk(&chunk) {
                Ok(encoded) if !encoded.is_empty() => {
                    self.pending.push_back(Ok(Bytes::from(encoded)));
                }
                Ok(_) => {}
                Err(err) => {
                    self.pending
                        .push_back(Err(ProviderError::Decode(err.to_string())));
                }
            }
        }
    }

    fn push_done(&mut self) {
        if self.done_emitted {
            return;
        }
        self.done_emitted = true;
        self.pending
            .push_back(Ok(Bytes::from(self.openai.encode_stream_done())));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn official_anthropic_uses_x_api_key_auth() {
        let provider =
            AnthropicProvider::new("p", "p", "https://api.anthropic.com", "sk-ant").unwrap();
        let headers = provider.auth_headers().unwrap();

        assert_eq!(headers.get("x-api-key").unwrap(), "sk-ant");
        assert!(headers.get(AUTHORIZATION).is_none());
    }

    #[test]
    fn relay_base_urls_use_bearer_auth() {
        let provider =
            AnthropicProvider::new("p", "p", "https://router.shengsuanyun.com/api", "sk-relay")
                .unwrap();
        let headers = provider.auth_headers().unwrap();

        assert_eq!(headers.get(AUTHORIZATION).unwrap(), "Bearer sk-relay");
        assert!(headers.get("x-api-key").is_none());
    }

    #[test]
    fn endpoint_deduplicates_v1_segments() {
        let provider =
            AnthropicProvider::new("p", "p", "https://api.anthropic.com/v1", "sk-ant").unwrap();

        assert_eq!(
            provider.endpoint("/v1/messages"),
            "https://api.anthropic.com/v1/messages",
        );
    }

    #[test]
    fn debug_does_not_leak_api_key() {
        let provider =
            AnthropicProvider::new("p", "p", "https://api.anthropic.com", "sk-secret").unwrap();
        let rendered = format!("{provider:?}");

        assert!(!rendered.contains("sk-secret"));
        assert!(rendered.contains("redacted"));
    }
}
