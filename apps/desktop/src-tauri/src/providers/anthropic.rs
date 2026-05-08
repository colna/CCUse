//! Native Anthropic Messages provider.
//!
//! `SwitchEngine` still dispatches provider-neutral [`ApiRequest`] values;
//! this provider serialises them to Anthropic's `/v1/messages` wire shape
//! and returns native Anthropic SSE for streaming calls.

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::converter::{
    AnthropicConverter, ContentPart, FinishReason, FormatConverter, Role, UnifiedMessage,
    UnifiedRequest, UnifiedResponse,
};

use super::api::{
    ApiChoice, ApiModel, ApiRequest, ApiResponse, ApiToolCall, ApiToolCallFunction, ChatContent,
    ChatContentPart, HealthStatus, Provider, ProviderError, StreamingResponse,
};
use super::error_format::format_reqwest_error;
use super::openai::DEFAULT_REQUEST_TIMEOUT;

const ANTHROPIC_VERSION: &str = "2023-06-01";

#[derive(Clone)]
pub struct AnthropicProvider {
    id: String,
    name: String,
    base_url: String,
    api_key: String,
    priority: i32,
    cost_per_token: Option<f64>,
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
            client,
            converter: AnthropicConverter::new(),
        })
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{path}", self.base_url.trim_end_matches('/'))
    }

    fn auth_headers(&self) -> Result<HeaderMap, ProviderError> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            "anthropic-version",
            HeaderValue::from_static(ANTHROPIC_VERSION),
        );
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
        let response = self
            .client
            .post(self.endpoint("/v1/messages"))
            .headers(self.auth_headers()?)
            .json(&minimal_health_body())
            .send()
            .await
            .map_err(|e| ProviderError::Network(format_reqwest_error(e)))?;

        match response.status() {
            s if s.is_success() => Ok(HealthStatus::Healthy),
            StatusCode::TOO_MANY_REQUESTS => Ok(HealthStatus::Degraded),
            _ => Ok(HealthStatus::Down),
        }
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
        let body = self.anthropic_body(&request, false)?;
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
        Err(map_http_error(status, body_text))
    }

    async fn send_stream_request(
        &self,
        request: ApiRequest,
    ) -> Result<StreamingResponse, ProviderError> {
        let body = self.anthropic_body(&request, true)?;
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
            return Err(map_http_error(status, body_text));
        }

        let upstream = response
            .bytes_stream()
            .map(|chunk| chunk.map_err(|e| ProviderError::Network(format_reqwest_error(e))));
        Ok(Box::pin(upstream))
    }
}

impl AnthropicProvider {
    fn anthropic_body(&self, request: &ApiRequest, stream: bool) -> Result<Value, ProviderError> {
        let mut unified = api_request_to_unified(request);
        unified.stream = stream;
        let mut body = self
            .converter
            .unified_to_request(&unified)
            .map_err(|e| ProviderError::BadRequest(e.to_string()))?;
        if let Some(object) = body.as_object_mut() {
            object.remove("model");
        }
        Ok(body)
    }
}

fn api_request_to_unified(request: &ApiRequest) -> UnifiedRequest {
    UnifiedRequest {
        model: request.model.clone(),
        messages: request
            .messages
            .iter()
            .map(chat_message_to_unified)
            .collect(),
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

fn chat_message_to_unified(message: &super::api::ChatMessage) -> UnifiedMessage {
    if let Some(tool_call_id) = &message.tool_call_id {
        return UnifiedMessage {
            role: Role::Tool,
            content: vec![ContentPart::ToolResult(crate::converter::ToolResult {
                tool_call_id: tool_call_id.clone(),
                output: message.content.text_content(),
            })],
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

fn minimal_health_body() -> Value {
    json!({
        "max_tokens": 1,
        "messages": [{"role": "user", "content": "ping"}],
    })
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

        assert_eq!(body.get("model"), None);
        assert_eq!(body["system"], "You are terse.");
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"][0]["text"], "ping");
        assert_eq!(body["tools"][0]["input_schema"]["type"], "object");
        assert_eq!(body["stream"], true);
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
