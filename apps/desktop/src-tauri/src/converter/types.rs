//! Unified intermediate format for cross-vendor API conversion.
//!
//! T1.0.3.01-02: These types sit between the client wire format and the
//! upstream provider wire format. Every converter translates *from* its
//! native format into [`UnifiedRequest`] / [`UnifiedResponse`] and back.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Helpers for safe JSON → primitive conversion (avoids `cast_possible_truncation`)
// ---------------------------------------------------------------------------

/// Safely extract a `u32` from a JSON value, saturating at `u32::MAX`.
#[must_use]
pub fn json_u32(val: &serde_json::Value) -> u32 {
    val.as_u64()
        .map_or(0, |v| u32::try_from(v).unwrap_or(u32::MAX))
}

/// Safely extract an `Option<u32>` from a JSON value.
#[must_use]
pub fn json_opt_u32(val: &serde_json::Value) -> Option<u32> {
    val.as_u64().map(|v| u32::try_from(v).unwrap_or(u32::MAX))
}

/// Safely extract an `Option<f32>` from a JSON value.
#[allow(clippy::cast_possible_truncation)]
#[must_use]
pub fn json_opt_f32(val: &serde_json::Value) -> Option<f32> {
    val.as_f64().map(|v| v as f32)
}

// ---------------------------------------------------------------------------
// Request
// ---------------------------------------------------------------------------

/// Vendor-neutral chat-completion request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnifiedRequest {
    pub model: String,
    pub messages: Vec<UnifiedMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    #[serde(default)]
    pub stream: bool,
    /// Tool definitions the model may call.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolDefinition>,
}

// ---------------------------------------------------------------------------
// Response
// ---------------------------------------------------------------------------

/// Vendor-neutral chat-completion response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnifiedResponse {
    pub id: String,
    pub model: String,
    pub choices: Vec<UnifiedChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<UnifiedUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnifiedChoice {
    pub index: u32,
    pub message: UnifiedMessage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<FinishReason>,
}

/// Normalised finish reasons across vendors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct UnifiedUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

// ---------------------------------------------------------------------------
// Message & Content (multimodal)
// ---------------------------------------------------------------------------

/// A single message in the conversation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnifiedMessage {
    pub role: Role,
    /// Content parts — text, images, tool calls, or tool results.
    pub content: Vec<ContentPart>,
    /// Optional sender name (for multi-turn with named participants).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl UnifiedMessage {
    /// Convenience: create a plain-text message.
    #[must_use]
    pub fn text(role: Role, text: impl Into<String>) -> Self {
        Self {
            role,
            content: vec![ContentPart::Text { text: text.into() }],
            name: None,
        }
    }

    /// Extract all text parts concatenated.
    #[must_use]
    pub fn text_content(&self) -> String {
        self.content
            .iter()
            .filter_map(|p| match p {
                ContentPart::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }

    /// Extract tool calls from content parts.
    #[must_use]
    pub fn tool_calls(&self) -> Vec<&ToolCall> {
        self.content
            .iter()
            .filter_map(|p| match p {
                ContentPart::ToolCall(tc) => Some(tc),
                _ => None,
            })
            .collect()
    }
}

/// Normalised role enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// A single content part inside a message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text {
        text: String,
    },
    ImageUrl {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    ToolCall(ToolCall),
    ToolResult(ToolResult),
}

// ---------------------------------------------------------------------------
// Tool calling
// ---------------------------------------------------------------------------

/// A tool invocation emitted by the model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    /// Unique id for correlating with the tool result.
    pub id: String,
    pub name: String,
    /// JSON-encoded arguments.
    pub arguments: String,
}

/// The result of executing a tool, sent back to the model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolResult {
    /// Must match a previous [`ToolCall::id`].
    pub tool_call_id: String,
    /// Serialised output (usually JSON or plain text).
    pub output: String,
}

/// A tool the model is allowed to invoke.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolDefinition {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON Schema for the parameters object.
    pub parameters: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Streaming
// ---------------------------------------------------------------------------

/// A single incremental chunk in a streaming response (T1.0.3.07).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnifiedStreamChunk {
    pub id: String,
    pub model: String,
    pub choices: Vec<StreamChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<UnifiedUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StreamChoice {
    pub index: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delta: Option<StreamDelta>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<FinishReason>,
}

/// Incremental content delta.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StreamDelta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<StreamToolCall>,
}

/// Incremental tool call delta in a stream.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StreamToolCall {
    pub index: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_message_roundtrip() {
        let msg = UnifiedMessage::text(Role::User, "hello");
        assert_eq!(msg.text_content(), "hello");
        assert!(msg.tool_calls().is_empty());
    }

    #[test]
    fn multimodal_message() {
        let msg = UnifiedMessage {
            role: Role::User,
            content: vec![
                ContentPart::Text {
                    text: "describe this".into(),
                },
                ContentPart::ImageUrl {
                    url: "https://example.com/img.png".into(),
                    detail: Some("high".into()),
                },
            ],
            name: None,
        };
        assert_eq!(msg.text_content(), "describe this");
    }

    #[test]
    fn tool_call_extraction() {
        let msg = UnifiedMessage {
            role: Role::Assistant,
            content: vec![
                ContentPart::Text {
                    text: "Let me search.".into(),
                },
                ContentPart::ToolCall(ToolCall {
                    id: "call_1".into(),
                    name: "search".into(),
                    arguments: r#"{"q":"rust"}"#.into(),
                }),
            ],
            name: None,
        };
        let calls = msg.tool_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "search");
    }

    #[test]
    fn unified_request_serialization() {
        let req = UnifiedRequest {
            model: "gpt-5.5-instant".into(),
            messages: vec![UnifiedMessage::text(Role::User, "hi")],
            temperature: Some(0.7),
            max_tokens: Some(100),
            top_p: None,
            stop: None,
            stream: false,
            tools: vec![],
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: UnifiedRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, back);
    }

    #[test]
    fn unified_response_serialization() {
        let resp = UnifiedResponse {
            id: "resp_1".into(),
            model: "gpt-5.5-instant".into(),
            choices: vec![UnifiedChoice {
                index: 0,
                message: UnifiedMessage::text(Role::Assistant, "hi there"),
                finish_reason: Some(FinishReason::Stop),
            }],
            usage: Some(UnifiedUsage {
                prompt_tokens: 5,
                completion_tokens: 3,
                total_tokens: 8,
            }),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let back: UnifiedResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp, back);
    }

    #[test]
    fn finish_reason_serde() {
        assert_eq!(
            serde_json::to_string(&FinishReason::ToolCalls).unwrap(),
            "\"tool_calls\""
        );
        let fr: FinishReason = serde_json::from_str("\"content_filter\"").unwrap();
        assert_eq!(fr, FinishReason::ContentFilter);
    }

    #[test]
    fn stream_chunk_serialization() {
        let chunk = UnifiedStreamChunk {
            id: "chunk_1".into(),
            model: "gpt-5.5-instant".into(),
            choices: vec![StreamChoice {
                index: 0,
                delta: Some(StreamDelta {
                    role: Some(Role::Assistant),
                    content: Some("Hello".into()),
                    tool_calls: vec![],
                }),
                finish_reason: None,
            }],
            usage: None,
        };
        let json = serde_json::to_string(&chunk).unwrap();
        let back: UnifiedStreamChunk = serde_json::from_str(&json).unwrap();
        assert_eq!(chunk, back);
    }

    #[test]
    fn tool_definition_with_schema() {
        let tool = ToolDefinition {
            name: "get_weather".into(),
            description: Some("Get weather for a location".into()),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "location": {"type": "string"}
                },
                "required": ["location"]
            }),
        };
        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("get_weather"));
        let back: ToolDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(tool, back);
    }

    #[test]
    fn content_part_tagged_serde() {
        let part = ContentPart::ToolCall(ToolCall {
            id: "tc_1".into(),
            name: "calc".into(),
            arguments: "{}".into(),
        });
        let json = serde_json::to_string(&part).unwrap();
        assert!(json.contains("\"type\":\"tool_call\""));
        let back: ContentPart = serde_json::from_str(&json).unwrap();
        assert_eq!(part, back);
    }
}
