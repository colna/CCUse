//! Native Google Gemini provider.

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE},
    Client, StatusCode,
};
use serde_json::Value;

use crate::converter::{FormatConverter, GeminiConverter, UnifiedRequest, UnifiedResponse};

use super::api::{
    ApiChoice, ApiModel, ApiRequest, ApiResponse, ChatContent, ChatContentPart, HealthStatus,
    Provider, ProviderError, StreamingResponse,
};
use super::error_format::format_reqwest_error;
use super::openai::DEFAULT_REQUEST_TIMEOUT;
use super::stream_check::{check_provider_with_default_config, health_status_from_stream_result};

#[derive(Clone)]
pub struct GeminiProvider {
    id: String,
    name: String,
    base_url: String,
    api_key: String,
    priority: i32,
    cost_per_token: Option<f64>,
    client: Client,
    converter: GeminiConverter,
}

impl std::fmt::Debug for GeminiProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GeminiProvider")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("base_url", &self.base_url)
            .field("api_key", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl GeminiProvider {
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
            converter: GeminiConverter::new(),
        })
    }

    fn endpoint_for_model(&self, model: &str, stream: bool) -> String {
        let base = self.base_url.trim_end_matches('/');
        let model = normalize_model_id(model);
        let method = if stream {
            "streamGenerateContent?alt=sse"
        } else {
            "generateContent"
        };
        if base.contains("/v1beta") || base.contains("/v1/") {
            format!("{base}/models/{model}:{method}")
        } else {
            format!("{base}/v1beta/models/{model}:{method}")
        }
    }

    fn auth_headers(&self) -> Result<HeaderMap, ProviderError> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            HeaderName::from_static("x-goog-api-key"),
            HeaderValue::from_str(&self.api_key).map_err(|e| {
                ProviderError::BadRequest(format!("api key contains invalid bytes: {e}"))
            })?,
        );
        Ok(headers)
    }

    fn stream_check_provider(&self) -> super::model::Provider {
        super::model::Provider {
            id: self.id.clone(),
            name: self.name.clone(),
            kind: super::model::ProviderKind::Gemini,
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

#[async_trait]
impl Provider for GeminiProvider {
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
        let url = append_endpoint(&self.base_url, "/v1beta/models");
        let response = self
            .client
            .get(url)
            .headers(self.auth_headers()?)
            .send()
            .await
            .map_err(|e| ProviderError::Network(format_reqwest_error(e)))?;
        let status = response.status();
        if status.is_success() {
            let body = response
                .json::<Value>()
                .await
                .map_err(|e| ProviderError::Decode(format_reqwest_error(e)))?;
            return Ok(body["models"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|model| {
                    let id = model["name"].as_str()?.strip_prefix("models/")?.to_owned();
                    Some(ApiModel {
                        id,
                        object: "model".to_owned(),
                        owned_by: Some("google".to_owned()),
                    })
                })
                .collect());
        }

        let body = response.text().await.unwrap_or_default();
        Err(map_http_error(status, body))
    }

    async fn send_request(&self, request: ApiRequest) -> Result<ApiResponse, ProviderError> {
        let body = self.gemini_body(&request)?;
        let response = self
            .client
            .post(self.endpoint_for_model(&request.model, false))
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

        let body = response.text().await.unwrap_or_default();
        Err(map_http_error(status, body))
    }

    async fn send_stream_request(
        &self,
        request: ApiRequest,
    ) -> Result<StreamingResponse, ProviderError> {
        let body = self.gemini_body(&request)?;
        let response = self
            .client
            .post(self.endpoint_for_model(&request.model, true))
            .headers(self.auth_headers()?)
            .header("accept", "text/event-stream")
            .header("accept-encoding", "identity")
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Network(format_reqwest_error(e)))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(map_http_error(status, body));
        }

        let upstream = response
            .bytes_stream()
            .map(|chunk| chunk.map_err(|e| ProviderError::Network(format_reqwest_error(e))));
        Ok(Box::pin(upstream))
    }
}

impl GeminiProvider {
    fn gemini_body(&self, request: &ApiRequest) -> Result<Value, ProviderError> {
        self.converter
            .unified_to_request(&api_request_to_unified(request))
            .map_err(|e| ProviderError::BadRequest(e.to_string()))
    }
}

fn api_request_to_unified(request: &ApiRequest) -> UnifiedRequest {
    UnifiedRequest {
        model: request.model.clone(),
        messages: request
            .messages
            .iter()
            .map(|message| crate::converter::UnifiedMessage {
                role: match message.role.as_str() {
                    "assistant" => crate::converter::Role::Assistant,
                    "system" => crate::converter::Role::System,
                    _ => crate::converter::Role::User,
                },
                content: match &message.content {
                    ChatContent::Text(text) => {
                        vec![crate::converter::ContentPart::Text { text: text.clone() }]
                    }
                    ChatContent::Parts(parts) => parts
                        .iter()
                        .map(|part| match part {
                            ChatContentPart::Text { text } => {
                                crate::converter::ContentPart::Text { text: text.clone() }
                            }
                            ChatContentPart::ImageUrl { image_url } => {
                                crate::converter::ContentPart::ImageUrl {
                                    url: image_url.url.clone(),
                                    detail: image_url.detail.clone(),
                                }
                            }
                        })
                        .collect(),
                },
                name: None,
            })
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

fn unified_response_to_api_response(response: &UnifiedResponse) -> ApiResponse {
    ApiResponse {
        id: response.id.clone(),
        model: response.model.clone(),
        choices: response
            .choices
            .iter()
            .map(|choice| ApiChoice {
                index: choice.index,
                message: super::api::ChatMessage {
                    role: "assistant".to_owned(),
                    content: choice.message.text_content().into(),
                    tool_call_id: None,
                    tool_calls: vec![],
                },
                finish_reason: choice
                    .finish_reason
                    .map(|reason| match reason {
                        crate::converter::FinishReason::Stop => "stop",
                        crate::converter::FinishReason::Length => "length",
                        crate::converter::FinishReason::ToolCalls => "tool_calls",
                        crate::converter::FinishReason::ContentFilter => "content_filter",
                    })
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

fn append_endpoint(base_url: &str, endpoint: &str) -> String {
    let base = base_url.trim_end_matches('/');
    if base.ends_with("/v1beta") {
        format!("{base}/{}", endpoint.trim_start_matches("/v1beta/"))
    } else {
        format!("{base}{endpoint}")
    }
}

fn normalize_model_id(model: &str) -> &str {
    model.strip_prefix("models/").unwrap_or(model)
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
    use super::*;
    use crate::providers::api::ChatMessage;

    #[test]
    fn debug_does_not_leak_api_key() {
        let provider = GeminiProvider::new(
            "gemini",
            "Gemini",
            "https://generativelanguage.googleapis.com",
            "sk-gemini",
        )
        .expect("provider");

        let rendered = format!("{provider:?}");

        assert!(
            !rendered.contains("sk-gemini"),
            "api key leaked: {rendered}"
        );
        assert!(rendered.contains("redacted"));
    }

    #[test]
    fn endpoint_for_model_uses_native_stream_generate_content_url() {
        let provider = GeminiProvider::new(
            "gemini",
            "Gemini",
            "https://generativelanguage.googleapis.com",
            "sk-gemini",
        )
        .expect("provider");

        assert_eq!(
            provider.endpoint_for_model("models/gemini-3-flash-preview", true),
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-3-flash-preview:streamGenerateContent?alt=sse"
        );
    }

    #[test]
    fn gemini_body_converts_openai_shape_to_native_contents() {
        let provider =
            GeminiProvider::new("gemini", "Gemini", "https://example.com", "sk").expect("provider");
        let body = provider
            .gemini_body(&ApiRequest {
                model: "gemini-3-flash-preview".to_owned(),
                messages: vec![ChatMessage {
                    role: "user".to_owned(),
                    content: "Who are you?".into(),
                    tool_call_id: None,
                    tool_calls: vec![],
                }],
                temperature: None,
                max_tokens: Some(1),
                stream: true,
                tools: vec![],
            })
            .expect("body");

        assert_eq!(body["contents"][0]["role"], "user");
        assert_eq!(body["contents"][0]["parts"][0]["text"], "Who are you?");
        assert_eq!(body["generationConfig"]["maxOutputTokens"], 1);
    }
}
