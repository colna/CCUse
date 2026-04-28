//! Anthropic Messages API format converter (T1.0.3.04 + T1.0.3.09).
//!
//! Key differences from `OpenAI`:
//! - `system` is a top-level field, not a message
//! - `content` is always an array of blocks (`text`, `image`, `tool_use`, `tool_result`)
//! - Tool calls use `tool_use` blocks with `input` (object, not string)
//! - Tool results use `tool_result` blocks
//! - SSE events: `message_start`, `content_block_start`, `content_block_delta`,
//!   `content_block_stop`, `message_delta`, `message_stop`

use std::fmt::Write as _;

use serde_json::{json, Value};

use super::types::{
    json_opt_f32, json_opt_u32, json_u32, ContentPart, FinishReason, Role, StreamChoice,
    StreamDelta, StreamToolCall, ToolCall, ToolDefinition, ToolResult, UnifiedChoice,
    UnifiedMessage, UnifiedRequest, UnifiedResponse, UnifiedStreamChunk, UnifiedUsage,
};
use super::{ConvertError, FormatConverter};

/// Converter for the Anthropic Messages API format.
#[derive(Debug, Clone, Default)]
pub struct AnthropicConverter;

impl AnthropicConverter {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    fn parse_role(s: &str) -> Result<Role, ConvertError> {
        match s {
            "user" => Ok(Role::User),
            "assistant" => Ok(Role::Assistant),
            other => Err(ConvertError::InvalidRole(other.into())),
        }
    }

    fn role_str(role: Role) -> &'static str {
        match role {
            Role::User | Role::System | Role::Tool => "user",
            Role::Assistant => "assistant",
        }
    }

    fn parse_stop_reason(s: &str) -> Option<FinishReason> {
        match s {
            "end_turn" => Some(FinishReason::Stop),
            "max_tokens" => Some(FinishReason::Length),
            "tool_use" => Some(FinishReason::ToolCalls),
            _ => None,
        }
    }

    fn stop_reason_str(fr: FinishReason) -> &'static str {
        match fr {
            FinishReason::Length => "max_tokens",
            FinishReason::ToolCalls => "tool_use",
            FinishReason::Stop | FinishReason::ContentFilter => "end_turn",
        }
    }

    fn parse_content_blocks(blocks: &[Value]) -> Vec<ContentPart> {
        let mut parts = Vec::new();
        for block in blocks {
            match block["type"].as_str() {
                Some("text") => {
                    let text = block["text"].as_str().unwrap_or_default().to_string();
                    parts.push(ContentPart::Text { text });
                }
                Some("image") => {
                    let media_type = block["source"]["media_type"]
                        .as_str()
                        .unwrap_or("image/png");
                    let data = block["source"]["data"].as_str().unwrap_or_default();
                    let url = format!("data:{media_type};base64,{data}");
                    parts.push(ContentPart::ImageUrl { url, detail: None });
                }
                Some("tool_use") => {
                    let id = block["id"].as_str().unwrap_or_default().to_string();
                    let name = block["name"].as_str().unwrap_or_default().to_string();
                    let arguments = serde_json::to_string(&block["input"])
                        .unwrap_or_else(|_| "{}".into());
                    parts.push(ContentPart::ToolCall(ToolCall {
                        id,
                        name,
                        arguments,
                    }));
                }
                Some("tool_result") => {
                    let tool_use_id = block["tool_use_id"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string();
                    let output = match &block["content"] {
                        Value::String(s) => s.clone(),
                        Value::Array(arr) => arr
                            .iter()
                            .filter_map(|b| b["text"].as_str())
                            .collect::<Vec<_>>()
                            .join(""),
                        _ => String::new(),
                    };
                    parts.push(ContentPart::ToolResult(ToolResult {
                        tool_call_id: tool_use_id,
                        output,
                    }));
                }
                _ => {}
            }
        }
        parts
    }

    fn content_parts_to_blocks(parts: &[ContentPart]) -> Vec<Value> {
        parts
            .iter()
            .map(|p| match p {
                ContentPart::Text { text } => json!({"type": "text", "text": text}),
                ContentPart::ImageUrl { url, .. } => {
                    if let Some(rest) = url.strip_prefix("data:") {
                        if let Some((media_type, data)) = rest.split_once(";base64,") {
                            return json!({
                                "type": "image",
                                "source": {
                                    "type": "base64",
                                    "media_type": media_type,
                                    "data": data,
                                }
                            });
                        }
                    }
                    json!({
                        "type": "image",
                        "source": { "type": "url", "url": url }
                    })
                }
                ContentPart::ToolCall(tc) => {
                    let input: Value =
                        serde_json::from_str(&tc.arguments).unwrap_or(json!({}));
                    json!({
                        "type": "tool_use",
                        "id": tc.id,
                        "name": tc.name,
                        "input": input,
                    })
                }
                ContentPart::ToolResult(tr) => {
                    json!({
                        "type": "tool_result",
                        "tool_use_id": tr.tool_call_id,
                        "content": tr.output,
                    })
                }
            })
            .collect()
    }

    // -- streaming helpers (split out to keep `parse_stream_chunk` under 100 lines) --

    fn parse_message_start(val: &Value) -> UnifiedStreamChunk {
        let msg = &val["message"];
        UnifiedStreamChunk {
            id: msg["id"].as_str().unwrap_or("").to_string(),
            model: msg["model"].as_str().unwrap_or("").to_string(),
            choices: vec![StreamChoice {
                index: 0,
                delta: Some(StreamDelta {
                    role: Some(Role::Assistant),
                    content: None,
                    tool_calls: vec![],
                }),
                finish_reason: None,
            }],
            usage: None,
        }
    }

    fn parse_content_block_start(val: &Value) -> Option<UnifiedStreamChunk> {
        let block = &val["content_block"];
        let index = json_u32(&val["index"]);
        match block["type"].as_str() {
            Some("text") => Some(UnifiedStreamChunk {
                id: String::new(),
                model: String::new(),
                choices: vec![StreamChoice {
                    index: 0,
                    delta: Some(StreamDelta {
                        role: None,
                        content: block["text"].as_str().map(String::from),
                        tool_calls: vec![],
                    }),
                    finish_reason: None,
                }],
                usage: None,
            }),
            Some("tool_use") => Some(UnifiedStreamChunk {
                id: String::new(),
                model: String::new(),
                choices: vec![StreamChoice {
                    index: 0,
                    delta: Some(StreamDelta {
                        role: None,
                        content: None,
                        tool_calls: vec![StreamToolCall {
                            index,
                            id: block["id"].as_str().map(String::from),
                            name: block["name"].as_str().map(String::from),
                            arguments: None,
                        }],
                    }),
                    finish_reason: None,
                }],
                usage: None,
            }),
            _ => None,
        }
    }

    fn parse_content_block_delta(val: &Value) -> Option<UnifiedStreamChunk> {
        let delta = &val["delta"];
        let index = json_u32(&val["index"]);
        match delta["type"].as_str() {
            Some("text_delta") => Some(UnifiedStreamChunk {
                id: String::new(),
                model: String::new(),
                choices: vec![StreamChoice {
                    index: 0,
                    delta: Some(StreamDelta {
                        role: None,
                        content: delta["text"].as_str().map(String::from),
                        tool_calls: vec![],
                    }),
                    finish_reason: None,
                }],
                usage: None,
            }),
            Some("input_json_delta") => Some(UnifiedStreamChunk {
                id: String::new(),
                model: String::new(),
                choices: vec![StreamChoice {
                    index: 0,
                    delta: Some(StreamDelta {
                        role: None,
                        content: None,
                        tool_calls: vec![StreamToolCall {
                            index,
                            id: None,
                            name: None,
                            arguments: delta["partial_json"].as_str().map(String::from),
                        }],
                    }),
                    finish_reason: None,
                }],
                usage: None,
            }),
            _ => None,
        }
    }

    fn parse_message_delta(val: &Value) -> UnifiedStreamChunk {
        let stop_reason = val["delta"]["stop_reason"]
            .as_str()
            .and_then(Self::parse_stop_reason);
        let usage = if val["usage"].is_object() {
            let out = json_u32(&val["usage"]["output_tokens"]);
            Some(UnifiedUsage {
                prompt_tokens: 0,
                completion_tokens: out,
                total_tokens: out,
            })
        } else {
            None
        };
        UnifiedStreamChunk {
            id: String::new(),
            model: String::new(),
            choices: vec![StreamChoice {
                index: 0,
                delta: None,
                finish_reason: stop_reason,
            }],
            usage,
        }
    }
}

impl FormatConverter for AnthropicConverter {
    fn request_to_unified(&self, body: &Value) -> Result<UnifiedRequest, ConvertError> {
        let model = body["model"]
            .as_str()
            .ok_or_else(|| ConvertError::MissingField("model".into()))?
            .to_string();

        let mut messages = Vec::new();

        // Anthropic puts system as a top-level field.
        if let Some(system) = body["system"].as_str() {
            messages.push(UnifiedMessage::text(Role::System, system));
        } else if let Value::Array(sys_blocks) = &body["system"] {
            let text = sys_blocks
                .iter()
                .filter_map(|b| b["text"].as_str())
                .collect::<Vec<_>>()
                .join("");
            if !text.is_empty() {
                messages.push(UnifiedMessage::text(Role::System, text));
            }
        }

        let msg_arr = body["messages"]
            .as_array()
            .ok_or_else(|| ConvertError::MissingField("messages".into()))?;

        for msg in msg_arr {
            let role_str = msg["role"]
                .as_str()
                .ok_or_else(|| ConvertError::MissingField("role".into()))?;
            let role = Self::parse_role(role_str)?;

            let content = match &msg["content"] {
                Value::String(s) => vec![ContentPart::Text { text: s.clone() }],
                Value::Array(arr) => Self::parse_content_blocks(arr),
                _ => vec![],
            };

            messages.push(UnifiedMessage {
                role,
                content,
                name: None,
            });
        }

        Ok(UnifiedRequest {
            model,
            messages,
            temperature: json_opt_f32(&body["temperature"]),
            max_tokens: json_opt_u32(&body["max_tokens"]),
            top_p: json_opt_f32(&body["top_p"]),
            stop: body["stop_sequences"].as_array().map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            }),
            stream: body["stream"].as_bool().unwrap_or(false),
            tools: body["tools"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|t| {
                            Some(ToolDefinition {
                                name: t["name"].as_str()?.to_string(),
                                description: t["description"].as_str().map(String::from),
                                parameters: t["input_schema"].clone(),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default(),
        })
    }

    fn unified_to_request(&self, req: &UnifiedRequest) -> Result<Value, ConvertError> {
        let system_text: String = req
            .messages
            .iter()
            .filter(|m| m.role == Role::System)
            .map(UnifiedMessage::text_content)
            .collect::<Vec<_>>()
            .join("\n");

        let messages: Vec<Value> = req
            .messages
            .iter()
            .filter(|m| m.role != Role::System)
            .map(|m| {
                let blocks = Self::content_parts_to_blocks(&m.content);
                json!({
                    "role": Self::role_str(m.role),
                    "content": blocks,
                })
            })
            .collect();

        let mut body = json!({
            "model": req.model,
            "messages": messages,
            "max_tokens": req.max_tokens.unwrap_or(4096),
        });

        if !system_text.is_empty() {
            body["system"] = json!(system_text);
        }
        if let Some(t) = req.temperature {
            body["temperature"] = json!(t);
        }
        if let Some(p) = req.top_p {
            body["top_p"] = json!(p);
        }
        if let Some(s) = &req.stop {
            body["stop_sequences"] = json!(s);
        }
        if req.stream {
            body["stream"] = json!(true);
        }
        if !req.tools.is_empty() {
            let tools: Vec<Value> = req
                .tools
                .iter()
                .map(|t| {
                    let mut tool = json!({
                        "name": t.name,
                        "input_schema": t.parameters,
                    });
                    if let Some(desc) = &t.description {
                        tool["description"] = json!(desc);
                    }
                    tool
                })
                .collect();
            body["tools"] = json!(tools);
        }

        Ok(body)
    }

    fn response_to_unified(&self, body: &Value) -> Result<UnifiedResponse, ConvertError> {
        let id = body["id"].as_str().unwrap_or("").to_string();
        let model = body["model"].as_str().unwrap_or("").to_string();

        let content_blocks = body["content"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let parts = Self::parse_content_blocks(&content_blocks);

        let finish_reason = body["stop_reason"]
            .as_str()
            .and_then(Self::parse_stop_reason);

        let usage = if body["usage"].is_object() {
            let input_t = json_u32(&body["usage"]["input_tokens"]);
            let output_t = json_u32(&body["usage"]["output_tokens"]);
            Some(UnifiedUsage {
                prompt_tokens: input_t,
                completion_tokens: output_t,
                total_tokens: input_t.saturating_add(output_t),
            })
        } else {
            None
        };

        Ok(UnifiedResponse {
            id,
            model,
            choices: vec![UnifiedChoice {
                index: 0,
                message: UnifiedMessage {
                    role: Role::Assistant,
                    content: parts,
                    name: None,
                },
                finish_reason,
            }],
            usage,
        })
    }

    fn unified_to_response(&self, resp: &UnifiedResponse) -> Result<Value, ConvertError> {
        let choice = resp.choices.first().ok_or_else(|| {
            ConvertError::MissingField("choices".into())
        })?;

        let content = Self::content_parts_to_blocks(&choice.message.content);

        let stop_reason = choice
            .finish_reason
            .map_or("end_turn", Self::stop_reason_str);

        let mut body = json!({
            "id": resp.id,
            "type": "message",
            "role": "assistant",
            "model": resp.model,
            "content": content,
            "stop_reason": stop_reason,
        });

        if let Some(u) = &resp.usage {
            body["usage"] = json!({
                "input_tokens": u.prompt_tokens,
                "output_tokens": u.completion_tokens,
            });
        }

        Ok(body)
    }

    fn parse_stream_chunk(&self, data: &str) -> Result<Option<UnifiedStreamChunk>, ConvertError> {
        let trimmed = data.trim();
        let val: Value = serde_json::from_str(trimmed)?;
        let event_type = val["type"].as_str().unwrap_or("");

        match event_type {
            "message_start" => Ok(Some(Self::parse_message_start(&val))),
            "content_block_start" => Ok(Self::parse_content_block_start(&val)),
            "content_block_delta" => Ok(Self::parse_content_block_delta(&val)),
            "message_delta" => Ok(Some(Self::parse_message_delta(&val))),
            _ => Ok(None),
        }
    }

    fn encode_stream_chunk(&self, chunk: &UnifiedStreamChunk) -> Result<String, ConvertError> {
        let mut frames = String::new();

        // If this chunk has id + model, emit message_start.
        if !chunk.id.is_empty() {
            let role_delta = chunk
                .choices
                .first()
                .and_then(|c| c.delta.as_ref())
                .and_then(|d| d.role);

            if role_delta.is_some() {
                let msg_start = json!({
                    "type": "message_start",
                    "message": {
                        "id": chunk.id,
                        "type": "message",
                        "role": "assistant",
                        "model": chunk.model,
                        "content": [],
                    }
                });
                let _ = write!(
                    frames,
                    "event: message_start\ndata: {}\n\n",
                    serde_json::to_string(&msg_start)?
                );
            }
        }

        for choice in &chunk.choices {
            if let Some(delta) = &choice.delta {
                if let Some(content) = &delta.content {
                    let event = json!({
                        "type": "content_block_delta",
                        "index": 0,
                        "delta": { "type": "text_delta", "text": content }
                    });
                    let _ = write!(
                        frames,
                        "event: content_block_delta\ndata: {}\n\n",
                        serde_json::to_string(&event)?
                    );
                }

                for tc in &delta.tool_calls {
                    if tc.id.is_some() || tc.name.is_some() {
                        let event = json!({
                            "type": "content_block_start",
                            "index": tc.index,
                            "content_block": {
                                "type": "tool_use",
                                "id": tc.id,
                                "name": tc.name,
                                "input": {},
                            }
                        });
                        let _ = write!(
                            frames,
                            "event: content_block_start\ndata: {}\n\n",
                            serde_json::to_string(&event)?
                        );
                    }
                    if let Some(args) = &tc.arguments {
                        let event = json!({
                            "type": "content_block_delta",
                            "index": tc.index,
                            "delta": { "type": "input_json_delta", "partial_json": args }
                        });
                        let _ = write!(
                            frames,
                            "event: content_block_delta\ndata: {}\n\n",
                            serde_json::to_string(&event)?
                        );
                    }
                }
            }

            if let Some(fr) = choice.finish_reason {
                let mut delta_event = json!({
                    "type": "message_delta",
                    "delta": { "stop_reason": Self::stop_reason_str(fr) }
                });
                if let Some(u) = &chunk.usage {
                    delta_event["usage"] = json!({"output_tokens": u.completion_tokens});
                }
                let _ = write!(
                    frames,
                    "event: message_delta\ndata: {}\n\n",
                    serde_json::to_string(&delta_event)?
                );
            }
        }

        Ok(frames)
    }

    fn encode_stream_done(&self) -> String {
        "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn converter() -> AnthropicConverter {
        AnthropicConverter::new()
    }

    #[test]
    fn simple_request_roundtrip() {
        let input = json!({
            "model": "claude-3-5-sonnet-20241022",
            "max_tokens": 1024,
            "system": "You are helpful.",
            "messages": [{"role": "user", "content": "hello"}]
        });
        let unified = converter().request_to_unified(&input).unwrap();
        assert_eq!(unified.model, "claude-3-5-sonnet-20241022");
        assert_eq!(unified.messages.len(), 2);
        assert_eq!(unified.messages[0].role, Role::System);
        assert_eq!(unified.messages[0].text_content(), "You are helpful.");
        assert_eq!(unified.messages[1].text_content(), "hello");

        let back = converter().unified_to_request(&unified).unwrap();
        assert_eq!(back["system"], "You are helpful.");
        assert_eq!(back["messages"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn tool_use_roundtrip() {
        let input = json!({
            "model": "claude-3-5-sonnet-20241022",
            "max_tokens": 1024,
            "messages": [
                {"role": "user", "content": "weather in Tokyo?"},
                {"role": "assistant", "content": [
                    {"type": "tool_use", "id": "tu_1", "name": "get_weather", "input": {"location": "Tokyo"}}
                ]},
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "tu_1", "content": "sunny 25C"}
                ]}
            ],
            "tools": [{
                "name": "get_weather",
                "description": "Get weather",
                "input_schema": {"type": "object", "properties": {"location": {"type": "string"}}}
            }]
        });
        let unified = converter().request_to_unified(&input).unwrap();
        assert_eq!(unified.messages.len(), 3);

        let tc = unified.messages[1].tool_calls();
        assert_eq!(tc.len(), 1);
        assert_eq!(tc[0].name, "get_weather");

        assert!(matches!(
            &unified.messages[2].content[0],
            ContentPart::ToolResult(tr) if tr.output == "sunny 25C"
        ));

        assert_eq!(unified.tools.len(), 1);

        let back = converter().unified_to_request(&unified).unwrap();
        assert_eq!(back["messages"][1]["content"][0]["type"], "tool_use");
    }

    #[test]
    fn response_roundtrip() {
        let input = json!({
            "id": "msg_abc",
            "type": "message",
            "role": "assistant",
            "model": "claude-3-5-sonnet-20241022",
            "content": [{"type": "text", "text": "Hello!"}],
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 10, "output_tokens": 5}
        });
        let unified = converter().response_to_unified(&input).unwrap();
        assert_eq!(unified.id, "msg_abc");
        assert_eq!(unified.choices[0].finish_reason, Some(FinishReason::Stop));
        assert_eq!(unified.usage.as_ref().unwrap().prompt_tokens, 10);
        assert_eq!(unified.usage.as_ref().unwrap().completion_tokens, 5);
        assert_eq!(unified.usage.as_ref().unwrap().total_tokens, 15);

        let back = converter().unified_to_response(&unified).unwrap();
        assert_eq!(back["stop_reason"], "end_turn");
        assert_eq!(back["content"][0]["text"], "Hello!");
    }

    #[test]
    fn stream_message_start() {
        let data = json!({
            "type": "message_start",
            "message": {
                "id": "msg_1",
                "type": "message",
                "role": "assistant",
                "model": "claude-3-5-sonnet-20241022",
                "content": []
            }
        });
        let chunk = converter()
            .parse_stream_chunk(&data.to_string())
            .unwrap()
            .unwrap();
        assert_eq!(chunk.id, "msg_1");
        assert_eq!(
            chunk.choices[0].delta.as_ref().unwrap().role,
            Some(Role::Assistant)
        );
    }

    #[test]
    fn stream_text_delta() {
        let data = json!({
            "type": "content_block_delta",
            "index": 0,
            "delta": {"type": "text_delta", "text": "Hello"}
        });
        let chunk = converter()
            .parse_stream_chunk(&data.to_string())
            .unwrap()
            .unwrap();
        assert_eq!(
            chunk.choices[0].delta.as_ref().unwrap().content.as_deref(),
            Some("Hello")
        );
    }

    #[test]
    fn stream_message_stop_returns_none() {
        let data = json!({"type": "message_stop"});
        let result = converter()
            .parse_stream_chunk(&data.to_string())
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn stream_tool_use_delta() {
        let start = json!({
            "type": "content_block_start",
            "index": 1,
            "content_block": {
                "type": "tool_use",
                "id": "tu_1",
                "name": "search",
                "input": {}
            }
        });
        let chunk = converter()
            .parse_stream_chunk(&start.to_string())
            .unwrap()
            .unwrap();
        let tc = &chunk.choices[0].delta.as_ref().unwrap().tool_calls;
        assert_eq!(tc.len(), 1);
        assert_eq!(tc[0].name.as_deref(), Some("search"));

        let delta = json!({
            "type": "content_block_delta",
            "index": 1,
            "delta": {"type": "input_json_delta", "partial_json": "{\"q\":\"rust\"}"}
        });
        let chunk2 = converter()
            .parse_stream_chunk(&delta.to_string())
            .unwrap()
            .unwrap();
        let tc2 = &chunk2.choices[0].delta.as_ref().unwrap().tool_calls;
        assert_eq!(tc2[0].arguments.as_deref(), Some("{\"q\":\"rust\"}"));
    }

    #[test]
    fn encode_stream_done() {
        let done = converter().encode_stream_done();
        assert!(done.contains("message_stop"));
    }

    #[test]
    fn image_base64_roundtrip() {
        let input = json!({
            "model": "claude-3-5-sonnet-20241022",
            "max_tokens": 1024,
            "messages": [{
                "role": "user",
                "content": [{
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": "image/png",
                        "data": "iVBOR..."
                    }
                }]
            }]
        });
        let unified = converter().request_to_unified(&input).unwrap();
        assert!(matches!(
            &unified.messages[0].content[0],
            ContentPart::ImageUrl { url, .. } if url.starts_with("data:image/png;base64,")
        ));

        let back = converter().unified_to_request(&unified).unwrap();
        assert_eq!(
            back["messages"][0]["content"][0]["source"]["media_type"],
            "image/png"
        );
    }
}
