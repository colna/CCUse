//! Bridge between [`UnifiedRequest`]/[`UnifiedResponse`] (converter layer)
//! and [`ApiRequest`]/[`ApiResponse`] (provider layer).
//!
//! All providers currently use `OpenAIProvider` which speaks `ApiRequest`.
//! The converter layer produces `UnifiedRequest`. This bridge maps between
//! the two, lossy for multimodal/tool-call content (text only for now).

use crate::converter::types::{
    FinishReason, Role, UnifiedChoice, UnifiedMessage, UnifiedResponse, UnifiedUsage,
};
use crate::converter::UnifiedRequest;
use crate::providers::api::{ApiRequest, ApiResponse, ChatMessage};

/// Convert a [`UnifiedRequest`] into the [`ApiRequest`] that
/// `SwitchEngine::dispatch` expects.
#[must_use]
pub fn unified_to_api_request(req: &UnifiedRequest) -> ApiRequest {
    ApiRequest {
        model: req.model.clone(),
        messages: req
            .messages
            .iter()
            .map(|m| ChatMessage {
                role: match m.role {
                    Role::System => "system",
                    Role::User => "user",
                    Role::Assistant => "assistant",
                    Role::Tool => "tool",
                }
                .to_owned(),
                content: m.text_content(),
            })
            .collect(),
        temperature: req.temperature,
        max_tokens: req.max_tokens,
        stream: req.stream,
    }
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
                message: UnifiedMessage::text(parse_role(&c.message.role), &c.message.content),
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

fn parse_role(role: &str) -> Role {
    match role {
        "system" => Role::System,
        "user" => Role::User,
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
            model: "gpt-4o".into(),
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

        assert_eq!(api.model, "gpt-4o");
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
    fn unified_to_api_request_multimodal_extracts_text_only() {
        let unified = UnifiedRequest {
            model: "gpt-4o".into(),
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
        assert_eq!(api.messages[0].content, "What is this?");
    }

    #[test]
    fn api_response_to_unified_round_trip() {
        let resp = ApiResponse {
            id: "chatcmpl-abc".into(),
            model: "gpt-4o".into(),
            choices: vec![ApiChoice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".into(),
                    content: "Hello!".into(),
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
        assert_eq!(unified.model, "gpt-4o");
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
}
