//! Native Anthropic Messages provider.
//!
//! `SwitchEngine` still dispatches provider-neutral [`ApiRequest`] values;
//! this provider serialises them to Anthropic's `/v1/messages` wire shape
//! and returns native Anthropic SSE for streaming calls.

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use serde_json::Value;

use crate::converter::{
    AnthropicConverter, ContentPart, FinishReason, FormatConverter, Role, ToolResult,
    UnifiedMessage, UnifiedRequest, UnifiedResponse,
};

use super::anthropic_headers::insert_claude_compatible_headers;
use super::api::{
    ApiChoice, ApiModel, ApiRequest, ApiResponse, ApiToolCall, ApiToolCallFunction, ChatContent,
    ChatContentPart, HealthStatus, Provider, ProviderError, StreamingResponse,
};
use super::default_models::{
    model_candidates, should_try_next_default_model, ANTHROPIC_DEFAULT_MODELS,
};
use super::error_format::format_reqwest_error;
use super::model;
use super::openai::DEFAULT_REQUEST_TIMEOUT;
use super::stream_check::{check_provider_with_default_config, health_status_from_stream_result};

#[derive(Clone)]
pub struct AnthropicProvider {
    id: String,
    name: String,
    base_url: String,
    api_key: String,
    priority: i32,
    cost_per_token: Option<f64>,
    default_models: Vec<String>,
    client: Client,
    converter: AnthropicConverter,
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    #[serde(default)]
    data: Vec<AnthropicModel>,
}

#[derive(Debug, Deserialize)]
struct AnthropicModel {
    id: String,
}

impl std::fmt::Debug for AnthropicProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnthropicProvider")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("base_url", &self.base_url)
            .field("api_key", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl AnthropicProvider {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        base_url: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Result<Self, ProviderError> {
        Self::with_options(id, name, base_url, api_key, 100, None)
    }

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
            ANTHROPIC_DEFAULT_MODELS
                .iter()
                .map(|model| (*model).to_owned())
                .collect(),
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
            converter: AnthropicConverter::new(),
        })
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{path}", self.base_url.trim_end_matches('/'))
    }

    fn auth_headers(&self) -> Result<HeaderMap, ProviderError> {
        let mut headers = HeaderMap::new();
        insert_claude_compatible_headers(&mut headers);
        let value = HeaderValue::from_str(&self.api_key).map_err(|e| {
            ProviderError::BadRequest(format!("api key contains invalid bytes: {e}"))
        })?;
        headers.insert("x-api-key", value);
        Ok(headers)
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
                .map(|body| {
                    body.data
                        .into_iter()
                        .map(|model| ApiModel {
                            id: model.id,
                            object: "model".to_owned(),
                            owned_by: Some("anthropic".to_owned()),
                        })
                        .collect()
                })
                .map_err(|e| ProviderError::Decode(format_reqwest_error(e)));
        }

        let body_text = response.text().await.unwrap_or_default();
        Err(map_http_error(status, body_text))
    }

    async fn send_request(&self, request: ApiRequest) -> Result<ApiResponse, ProviderError> {
        let candidates = model_candidates(&request.model, &self.default_models);
        let mut last_error = None;
        for model in candidates {
            let body = self.anthropic_body(&request_with_model(&request, &model), false)?;
            let response = self
                .client
                .post(self.endpoint("/v1/messages"))
                .headers(self.auth_headers()?)
                .json(&body)
                .send()
                .await
                .map_err(|e| ProviderError::Network(format_reqwest_error(e)))?;

            let status = response.status();
            if status.is_success() {
                let body = response
                    .json::<Value>()
                    .await
                    .map_err(|e| ProviderError::Decode(format_reqwest_error(e)))?;
                let unified = self
                    .converter
                    .response_to_unified(&body)
                    .map_err(|e| ProviderError::Decode(e.to_string()))?;
                return Ok(unified_response_to_api_response(&unified));
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
        let candidates = model_candidates(&request.model, &self.default_models);
        let mut last_error = None;
        for model in candidates {
            let body = self.anthropic_body(&request_with_model(&request, &model), true)?;
            let response = self
                .client
                .post(self.endpoint("/v1/messages"))
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

impl AnthropicProvider {
    fn stream_check_provider(&self) -> model::Provider {
        model::Provider {
            id: self.id.clone(),
            name: self.name.clone(),
            kind: model::ProviderKind::Anthropic,
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

impl AnthropicProvider {
    fn anthropic_body(&self, request: &ApiRequest, stream: bool) -> Result<Value, ProviderError> {
        let mut unified = api_request_to_unified(request);
        unified.stream = stream;
        let body = self
            .converter
            .unified_to_request(&unified)
            .map_err(|e| ProviderError::BadRequest(e.to_string()))?;
        Ok(body)
    }
}

fn api_request_to_unified(request: &ApiRequest) -> UnifiedRequest {
    UnifiedRequest {
        model: request.model.clone(),
        messages: chat_messages_to_unified(&request.messages),
        temperature: request.temperature,
        max_tokens: request.max_tokens,
        top_p: None,
        stop: None,
        stream: request.stream,
        tools: request
            .tools
            .iter()
            .map(|tool| crate::converter::ToolDefinition {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
            })
            .collect(),
    }
}

fn chat_messages_to_unified(messages: &[super::api::ChatMessage]) -> Vec<UnifiedMessage> {
    let mut unified = Vec::new();
    let mut pending_tool_results = Vec::new();

    for message in messages {
        if let Some(tool_result) = chat_message_tool_result(message) {
            pending_tool_results.push(ContentPart::ToolResult(tool_result));
            continue;
        }

        flush_pending_tool_results(&mut unified, &mut pending_tool_results);
        unified.push(chat_message_to_unified(message));
    }

    flush_pending_tool_results(&mut unified, &mut pending_tool_results);
    unified
}

fn flush_pending_tool_results(
    messages: &mut Vec<UnifiedMessage>,
    pending_tool_results: &mut Vec<ContentPart>,
) {
    if pending_tool_results.is_empty() {
        return;
    }

    messages.push(UnifiedMessage {
        role: Role::Tool,
        content: std::mem::take(pending_tool_results),
        name: None,
    });
}

fn chat_message_tool_result(message: &super::api::ChatMessage) -> Option<ToolResult> {
    message
        .tool_call_id
        .as_ref()
        .map(|tool_call_id| ToolResult {
            tool_call_id: tool_call_id.clone(),
            output: message.content.text_content(),
        })
}

fn request_with_model(request: &ApiRequest, model: &str) -> ApiRequest {
    let mut mapped = request.clone();
    model.clone_into(&mut mapped.model);
    mapped
}

fn chat_message_to_unified(message: &super::api::ChatMessage) -> UnifiedMessage {
    if let Some(result) = chat_message_tool_result(message) {
        return UnifiedMessage {
            role: Role::Tool,
            content: vec![ContentPart::ToolResult(result)],
            name: None,
        };
    }

    let mut content = chat_content_to_unified_parts(&message.content);
    content.extend(message.tool_calls.iter().map(|call| {
        ContentPart::ToolCall(crate::converter::ToolCall {
            id: call.id.clone(),
            name: call.function.name.clone(),
            arguments: call.function.arguments.clone(),
        })
    }));

    UnifiedMessage {
        role: parse_provider_role(&message.role),
        content,
        name: None,
    }
}

fn chat_content_to_unified_parts(content: &ChatContent) -> Vec<ContentPart> {
    match content {
        ChatContent::Text(text) => {
            if text.is_empty() {
                Vec::new()
            } else {
                vec![ContentPart::Text { text: text.clone() }]
            }
        }
        ChatContent::Parts(parts) => parts
            .iter()
            .map(|part| match part {
                ChatContentPart::Text { text } => ContentPart::Text { text: text.clone() },
                ChatContentPart::ImageUrl { image_url } => ContentPart::ImageUrl {
                    url: image_url.url.clone(),
                    detail: image_url.detail.clone(),
                },
            })
            .collect(),
    }
}

fn unified_response_to_api_response(response: &UnifiedResponse) -> ApiResponse {
    ApiResponse {
        id: response.id.clone(),
        model: response.model.clone(),
        choices: response
            .choices
            .iter()
            .map(|choice| ApiChoice {
                index: choice.index,
                message: unified_message_to_chat_message(&choice.message),
                finish_reason: choice
                    .finish_reason
                    .map(finish_reason_to_openai)
                    .map(str::to_owned),
            })
            .collect(),
        usage: response.usage.as_ref().map(|usage| super::api::ApiUsage {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
        }),
    }
}

fn unified_message_to_chat_message(message: &UnifiedMessage) -> super::api::ChatMessage {
    let mut content_parts = Vec::new();
    let mut tool_calls = Vec::new();

    for part in &message.content {
        match part {
            ContentPart::Text { text } => {
                content_parts.push(ChatContentPart::Text { text: text.clone() });
            }
            ContentPart::ImageUrl { url, detail } => {
                content_parts.push(ChatContentPart::ImageUrl {
                    image_url: super::api::ChatImageUrl {
                        url: url.clone(),
                        detail: detail.clone(),
                    },
                });
            }
            ContentPart::ToolCall(tool_call) => {
                tool_calls.push(ApiToolCall {
                    id: tool_call.id.clone(),
                    kind: "function".to_owned(),
                    function: ApiToolCallFunction {
                        name: tool_call.name.clone(),
                        arguments: tool_call.arguments.clone(),
                    },
                });
            }
            ContentPart::ToolResult(_) => {}
        }
    }

    let content = match content_parts.as_slice() {
        [ChatContentPart::Text { text }] => ChatContent::Text(text.clone()),
        [] => ChatContent::Text(String::new()),
        _ => ChatContent::parts(content_parts),
    };

    super::api::ChatMessage {
        role: "assistant".to_owned(),
        content,
        tool_call_id: None,
        tool_calls,
    }
}

fn parse_provider_role(role: &str) -> Role {
    match role {
        "system" => Role::System,
        "assistant" => Role::Assistant,
        "tool" => Role::Tool,
        _ => Role::User,
    }
}

fn finish_reason_to_openai(reason: FinishReason) -> &'static str {
    match reason {
        FinishReason::Stop => "stop",
        FinishReason::Length => "length",
        FinishReason::ToolCalls => "tool_calls",
        FinishReason::ContentFilter => "content_filter",
    }
}

fn map_http_error(status: StatusCode, body: String) -> ProviderError {
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
    use super::super::api::{ApiToolDefinition, ChatMessage};
    use super::*;
    use crate::converter::{UnifiedChoice, UnifiedUsage};
    use serde_json::json;

    fn provider() -> AnthropicProvider {
        AnthropicProvider::new(
            "anthropic",
            "Anthropic",
            "https://api.anthropic.com",
            "sk-ant",
        )
        .expect("provider")
    }

    #[test]
    fn debug_does_not_leak_api_key() {
        let rendered = format!("{:?}", provider());

        assert!(!rendered.contains("sk-ant"), "api key leaked: {rendered}");
        assert!(rendered.contains("redacted"));
    }

    #[test]
    fn auth_headers_include_claude_cli_compatibility_headers() {
        let headers = provider().auth_headers().expect("headers");

        assert_eq!(headers.get("x-api-key").expect("x-api-key"), "sk-ant");
        assert_eq!(
            headers.get("anthropic-version").expect("anthropic-version"),
            super::super::anthropic_headers::ANTHROPIC_VERSION,
        );
        assert_eq!(
            headers.get("user-agent").expect("user-agent"),
            super::super::anthropic_headers::CLAUDE_CLI_USER_AGENT,
        );
        assert_eq!(headers.get("x-stainless-lang").expect("lang"), "js");
        assert_eq!(headers.get("x-stainless-runtime").expect("runtime"), "node");
    }

    #[test]
    fn anthropic_body_uses_messages_endpoint_shape() {
        let body = provider()
            .anthropic_body(
                &ApiRequest {
                    model: "claude-sonnet-4-20250514".to_owned(),
                    messages: vec![
                        ChatMessage {
                            role: "system".to_owned(),
                            content: "You are terse.".into(),
                            tool_call_id: None,
                            tool_calls: vec![],
                        },
                        ChatMessage {
                            role: "user".to_owned(),
                            content: "ping".into(),
                            tool_call_id: None,
                            tool_calls: vec![],
                        },
                    ],
                    temperature: Some(0.2),
                    max_tokens: Some(64),
                    stream: false,
                    tools: vec![ApiToolDefinition {
                        name: "get_weather".to_owned(),
                        description: Some("Get weather".to_owned()),
                        parameters: json!({"type": "object"}),
                    }],
                },
                true,
            )
            .expect("body");

        assert_eq!(body["model"], "claude-sonnet-4-20250514");
        assert_eq!(body["system"], "You are terse.");
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"][0]["text"], "ping");
        assert_eq!(body["tools"][0]["input_schema"]["type"], "object");
        assert_eq!(body["stream"], true);
    }

    #[test]
    fn anthropic_body_groups_consecutive_tool_results_into_one_user_message() {
        let body = provider()
            .anthropic_body(
                &ApiRequest {
                    model: "claude-sonnet-4-20250514".to_owned(),
                    messages: vec![
                        ChatMessage {
                            role: "assistant".to_owned(),
                            content: ChatContent::default(),
                            tool_call_id: None,
                            tool_calls: vec![
                                ApiToolCall {
                                    id: "toolu_one".to_owned(),
                                    kind: "function".to_owned(),
                                    function: ApiToolCallFunction {
                                        name: "first".to_owned(),
                                        arguments: "{}".to_owned(),
                                    },
                                },
                                ApiToolCall {
                                    id: "toolu_two".to_owned(),
                                    kind: "function".to_owned(),
                                    function: ApiToolCallFunction {
                                        name: "second".to_owned(),
                                        arguments: "{}".to_owned(),
                                    },
                                },
                            ],
                        },
                        ChatMessage {
                            role: "tool".to_owned(),
                            content: "one".into(),
                            tool_call_id: Some("toolu_one".to_owned()),
                            tool_calls: vec![],
                        },
                        ChatMessage {
                            role: "tool".to_owned(),
                            content: "two".into(),
                            tool_call_id: Some("toolu_two".to_owned()),
                            tool_calls: vec![],
                        },
                    ],
                    temperature: None,
                    max_tokens: Some(64),
                    stream: false,
                    tools: vec![],
                },
                false,
            )
            .expect("body");

        assert_eq!(body["messages"].as_array().expect("messages").len(), 2);
        assert_eq!(body["messages"][1]["role"], "user");
        assert_eq!(
            body["messages"][1]["content"][0]["tool_use_id"],
            "toolu_one"
        );
        assert_eq!(
            body["messages"][1]["content"][1]["tool_use_id"],
            "toolu_two"
        );
    }

    #[test]
    fn response_conversion_preserves_tool_use() {
        let unified = UnifiedResponse {
            id: "msg_1".to_owned(),
            model: "claude-sonnet-4-20250514".to_owned(),
            choices: vec![UnifiedChoice {
                index: 0,
                message: UnifiedMessage {
                    role: Role::Assistant,
                    content: vec![ContentPart::ToolCall(crate::converter::ToolCall {
                        id: "toolu_1".to_owned(),
                        name: "get_weather".to_owned(),
                        arguments: "{\"city\":\"Tokyo\"}".to_owned(),
                    })],
                    name: None,
                },
                finish_reason: Some(FinishReason::ToolCalls),
            }],
            usage: Some(UnifiedUsage {
                prompt_tokens: 10,
                completion_tokens: 3,
                total_tokens: 13,
            }),
        };

        let response = unified_response_to_api_response(&unified);

        assert_eq!(
            response.choices[0].finish_reason.as_deref(),
            Some("tool_calls")
        );
        assert_eq!(response.choices[0].message.tool_calls[0].id, "toolu_1");
        assert_eq!(response.usage.expect("usage").total_tokens, 13);
    }
}
