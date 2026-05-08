//! Bridge between [`UnifiedRequest`]/[`UnifiedResponse`] (converter layer)
//! and [`ApiRequest`]/[`ApiResponse`] (provider layer).
//!
//! All providers currently use `OpenAIProvider` which speaks `ApiRequest`.
//! The converter layer produces `UnifiedRequest`. This bridge maps between
//! the two, preserving text, images, and OpenAI-compatible tool calls.

use crate::converter::types::{
    ContentPart, FinishReason, Role, ToolCall, ToolResult, UnifiedChoice, UnifiedMessage,
    UnifiedResponse, UnifiedUsage,
};
use crate::converter::UnifiedRequest;
use crate::providers::api::{
    ApiRequest, ApiResponse, ApiToolCall, ApiToolCallFunction, ApiToolDefinition, ChatContent,
    ChatContentPart, ChatImageUrl, ChatMessage,
};

/// Convert a [`UnifiedRequest`] into the [`ApiRequest`] that
/// `SwitchEngine::dispatch` expects.
#[must_use]
pub fn unified_to_api_request(req: &UnifiedRequest) -> ApiRequest {
    ApiRequest {
        model: req.model.clone(),
        messages: req
            .messages
            .iter()
            .flat_map(unified_message_to_chat_messages)
            .collect(),
        temperature: req.temperature,
        max_tokens: req.max_tokens,
        stream: req.stream,
        tools: req
            .tools
            .iter()
            .map(|tool| ApiToolDefinition {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
            })
            .collect(),
    }
}

fn unified_message_to_chat_messages(message: &UnifiedMessage) -> Vec<ChatMessage> {
    let mut messages = Vec::new();
    let mut has_tool_result = false;
    let mut has_regular_content = false;

    for part in &message.content {
        match part {
            ContentPart::ToolResult(result) => {
                has_tool_result = true;
                messages.push(tool_result_to_chat_message(result));
            }
            ContentPart::Text { .. } | ContentPart::ImageUrl { .. } | ContentPart::ToolCall(_) => {
                has_regular_content = true;
            }
        }
    }

    if !has_tool_result || has_regular_content {
        messages.push(regular_unified_message_to_chat(message));
    }

    messages
}

fn tool_result_to_chat_message(result: &ToolResult) -> ChatMessage {
    ChatMessage {
        role: "tool".to_owned(),
        content: result.output.as_str().into(),
        tool_call_id: Some(result.tool_call_id.clone()),
        tool_calls: vec![],
    }
}

fn regular_unified_message_to_chat(message: &UnifiedMessage) -> ChatMessage {
    let tool_calls = message
        .tool_calls()
        .into_iter()
        .map(|call| ApiToolCall {
            id: call.id.clone(),
            kind: "function".to_owned(),
            function: ApiToolCallFunction {
                name: call.name.clone(),
                arguments: call.arguments.clone(),
            },
        })
        .collect();

    ChatMessage {
        role: role_to_provider(message.role).to_owned(),
        content: unified_content_to_chat_content(&message.content),
        tool_call_id: None,
        tool_calls,
    }
}

fn unified_content_to_chat_content(parts: &[ContentPart]) -> ChatContent {
    let content_parts = parts
        .iter()
        .filter_map(|part| match part {
            ContentPart::Text { text } => Some(ChatContentPart::Text { text: text.clone() }),
            ContentPart::ImageUrl { url, detail } => Some(ChatContentPart::ImageUrl {
                image_url: ChatImageUrl {
                    url: url.clone(),
                    detail: detail.clone(),
                },
            }),
            ContentPart::ToolCall(_) | ContentPart::ToolResult(_) => None,
        })
        .collect::<Vec<_>>();

    if content_parts.is_empty() {
        return ChatContent::Text(String::new());
    }
    if let [ChatContentPart::Text { text }] = content_parts.as_slice() {
        return ChatContent::Text(text.clone());
    }

    ChatContent::parts(content_parts)
}

/// Convert an [`ApiResponse`] into a [`UnifiedResponse`].
#[must_use]
pub fn api_response_to_unified(resp: &ApiResponse) -> UnifiedResponse {
    UnifiedResponse {
        id: resp.id.clone(),
        model: resp.model.clone(),
        choices: resp
            .choices
            .iter()
            .map(|c| UnifiedChoice {
                index: c.index,
                message: chat_message_to_unified(&c.message),
                finish_reason: c.finish_reason.as_deref().and_then(parse_finish_reason),
            })
            .collect(),
        usage: resp.usage.as_ref().map(|u| UnifiedUsage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        }),
    }
}

fn chat_message_to_unified(message: &ChatMessage) -> UnifiedMessage {
    if let Some(tool_call_id) = &message.tool_call_id {
        return UnifiedMessage {
            role: Role::Tool,
            content: vec![ContentPart::ToolResult(ToolResult {
                tool_call_id: tool_call_id.clone(),
                output: message.content.text_content(),
            })],
            name: None,
        };
    }

    let mut content = Vec::new();
    if !message.content.is_empty() {
        match &message.content {
            ChatContent::Text(text) => content.push(ContentPart::Text { text: text.clone() }),
            ChatContent::Parts(parts) => content.extend(parts.iter().map(|part| match part {
                ChatContentPart::Text { text } => ContentPart::Text { text: text.clone() },
                ChatContentPart::ImageUrl { image_url } => ContentPart::ImageUrl {
                    url: image_url.url.clone(),
                    detail: image_url.detail.clone(),
                },
            })),
        }
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

fn role_to_provider(role: Role) -> &'static str {
    match role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::Tool => "tool",
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

fn parse_finish_reason(reason: &str) -> Option<FinishReason> {
    match reason {
        "stop" => Some(FinishReason::Stop),
        "length" => Some(FinishReason::Length),
        "tool_calls" => Some(FinishReason::ToolCalls),
        "content_filter" => Some(FinishReason::ContentFilter),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::converter::types::ContentPart;
    use crate::providers::api::{ApiChoice, ApiUsage};

    #[test]
    fn unified_to_api_request_maps_text_messages() {
        let unified = UnifiedRequest {
            model: "gpt-5.5-instant".into(),
            messages: vec![
                UnifiedMessage::text(Role::System, "You are helpful"),
                UnifiedMessage::text(Role::User, "Hello"),
            ],
            temperature: Some(0.7),
            max_tokens: Some(100),
            top_p: None,
            stop: None,
            stream: false,
            tools: vec![],
        };

        let api = unified_to_api_request(&unified);

        assert_eq!(api.model, "gpt-5.5-instant");
        assert_eq!(api.messages.len(), 2);
        assert_eq!(api.messages[0].role, "system");
        assert_eq!(api.messages[0].content, "You are helpful");
        assert_eq!(api.messages[1].role, "user");
        assert_eq!(api.messages[1].content, "Hello");
        assert_eq!(api.temperature, Some(0.7));
        assert_eq!(api.max_tokens, Some(100));
        assert!(!api.stream);
    }

    #[test]
    fn unified_to_api_request_preserves_multimodal_content() {
        let unified = UnifiedRequest {
            model: "gpt-5.5-instant".into(),
            messages: vec![UnifiedMessage {
                role: Role::User,
                content: vec![
                    ContentPart::Text {
                        text: "What is this?".into(),
                    },
                    ContentPart::ImageUrl {
                        url: "https://example.com/img.png".into(),
                        detail: None,
                    },
                ],
                name: None,
            }],
            temperature: None,
            max_tokens: None,
            top_p: None,
            stop: None,
            stream: false,
            tools: vec![],
        };

        let api = unified_to_api_request(&unified);
        let ChatContent::Parts(parts) = &api.messages[0].content else {
            panic!("expected multimodal content parts");
        };
        assert_eq!(parts.len(), 2);
        assert!(matches!(&parts[0], ChatContentPart::Text { text } if text == "What is this?"));
        assert!(
            matches!(&parts[1], ChatContentPart::ImageUrl { image_url } if image_url.url == "https://example.com/img.png")
        );
    }

    #[test]
    fn api_response_to_unified_round_trip() {
        let resp = ApiResponse {
            id: "chatcmpl-abc".into(),
            model: "gpt-5.5-instant".into(),
            choices: vec![ApiChoice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".into(),
                    content: "Hello!".into(),
                    tool_call_id: None,
                    tool_calls: vec![],
                },
                finish_reason: Some("stop".into()),
            }],
            usage: Some(ApiUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            }),
        };

        let unified = api_response_to_unified(&resp);

        assert_eq!(unified.id, "chatcmpl-abc");
        assert_eq!(unified.model, "gpt-5.5-instant");
        assert_eq!(unified.choices.len(), 1);
        assert_eq!(unified.choices[0].message.role, Role::Assistant);
        assert_eq!(unified.choices[0].message.text_content(), "Hello!");
        assert_eq!(unified.choices[0].finish_reason, Some(FinishReason::Stop));
        let usage = unified.usage.as_ref().unwrap();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 5);
    }

    #[test]
    fn api_response_without_usage_maps_to_none() {
        let resp = ApiResponse {
            id: "id".into(),
            model: "m".into(),
            choices: vec![],
            usage: None,
        };
        assert!(api_response_to_unified(&resp).usage.is_none());
    }

    #[test]
    fn unified_to_api_request_preserves_tool_definitions() {
        let unified = UnifiedRequest {
            model: "gpt-5.5-instant".into(),
            messages: vec![UnifiedMessage::text(Role::User, "weather?")],
            temperature: None,
            max_tokens: None,
            top_p: None,
            stop: None,
            stream: false,
            tools: vec![crate::converter::ToolDefinition {
                name: "get_weather".into(),
                description: Some("Get weather".into()),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {"city": {"type": "string"}}
                }),
            }],
        };

        let api = unified_to_api_request(&unified);

        assert_eq!(api.tools.len(), 1);
        assert_eq!(api.tools[0].name, "get_weather");
        assert_eq!(api.tools[0].description.as_deref(), Some("Get weather"));
        assert_eq!(api.tools[0].parameters["type"], "object");
    }

    #[test]
    fn unified_to_api_request_preserves_tool_call_and_result_messages() {
        let unified = UnifiedRequest {
            model: "gpt-5.5-instant".into(),
            messages: vec![
                UnifiedMessage {
                    role: Role::Assistant,
                    content: vec![ContentPart::ToolCall(crate::converter::ToolCall {
                        id: "call_weather".into(),
                        name: "get_weather".into(),
                        arguments: "{\"city\":\"Tokyo\"}".into(),
                    })],
                    name: None,
                },
                UnifiedMessage {
                    role: Role::User,
                    content: vec![ContentPart::ToolResult(crate::converter::ToolResult {
                        tool_call_id: "call_weather".into(),
                        output: "sunny".into(),
                    })],
                    name: None,
                },
            ],
            temperature: None,
            max_tokens: None,
            top_p: None,
            stop: None,
            stream: false,
            tools: vec![],
        };

        let api = unified_to_api_request(&unified);

        assert_eq!(api.messages[0].role, "assistant");
        assert_eq!(api.messages[0].tool_calls[0].id, "call_weather");
        assert_eq!(api.messages[0].tool_calls[0].function.name, "get_weather");
        assert_eq!(api.messages[1].role, "tool");
        assert_eq!(
            api.messages[1].tool_call_id.as_deref(),
            Some("call_weather")
        );
        assert_eq!(api.messages[1].content, "sunny");
    }

    #[test]
    fn unified_to_api_request_preserves_multiple_tool_results_from_one_message() {
        let unified = UnifiedRequest {
            model: "gpt-5.5-instant".into(),
            messages: vec![UnifiedMessage {
                role: Role::User,
                content: vec![
                    ContentPart::ToolResult(crate::converter::ToolResult {
                        tool_call_id: "toolu_one".into(),
                        output: "one".into(),
                    }),
                    ContentPart::ToolResult(crate::converter::ToolResult {
                        tool_call_id: "toolu_two".into(),
                        output: "two".into(),
                    }),
                ],
                name: None,
            }],
            temperature: None,
            max_tokens: None,
            top_p: None,
            stop: None,
            stream: false,
            tools: vec![],
        };

        let api = unified_to_api_request(&unified);

        assert_eq!(api.messages.len(), 2);
        assert_eq!(api.messages[0].tool_call_id.as_deref(), Some("toolu_one"));
        assert_eq!(api.messages[1].tool_call_id.as_deref(), Some("toolu_two"));
    }

    #[test]
    fn api_response_to_unified_preserves_assistant_tool_calls() {
        let resp = ApiResponse {
            id: "chatcmpl-tool".into(),
            model: "gpt-5.5-instant".into(),
            choices: vec![ApiChoice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".into(),
                    content: ChatContent::default(),
                    tool_call_id: None,
                    tool_calls: vec![crate::providers::ApiToolCall {
                        id: "call_weather".into(),
                        kind: "function".into(),
                        function: crate::providers::ApiToolCallFunction {
                            name: "get_weather".into(),
                            arguments: "{\"city\":\"Tokyo\"}".into(),
                        },
                    }],
                },
                finish_reason: Some("tool_calls".into()),
            }],
            usage: None,
        };

        let unified = api_response_to_unified(&resp);

        assert_eq!(
            unified.choices[0].finish_reason,
            Some(crate::converter::FinishReason::ToolCalls)
        );
        let calls = unified.choices[0].message.tool_calls();
        assert_eq!(calls[0].id, "call_weather");
        assert_eq!(calls[0].name, "get_weather");
    }

    #[test]
    fn api_response_to_unified_preserves_multimodal_content() {
        let resp = ApiResponse {
            id: "chatcmpl-image".into(),
            model: "gpt-5.5-instant".into(),
            choices: vec![ApiChoice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".into(),
                    content: ChatContent::parts(vec![
                        ChatContentPart::Text {
                            text: "diagram".into(),
                        },
                        ChatContentPart::ImageUrl {
                            image_url: ChatImageUrl {
                                url: "data:image/png;base64,abc".into(),
                                detail: Some("high".into()),
                            },
                        },
                    ]),
                    tool_call_id: None,
                    tool_calls: vec![],
                },
                finish_reason: Some("stop".into()),
            }],
            usage: None,
        };

        let unified = api_response_to_unified(&resp);

        assert!(matches!(
            &unified.choices[0].message.content[1],
            ContentPart::ImageUrl { url, detail } if url == "data:image/png;base64,abc" && detail.as_deref() == Some("high")
        ));
    }
}
