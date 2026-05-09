//! `OpenAI` format converter (T1.0.3.03 + T1.0.3.08).
//!
//! Maps between the `OpenAI` chat-completions wire format and the
//! unified intermediate representation.  Because `CCUse`'s unified
//! format is modelled closely on `OpenAI`, this converter is largely a
//! pass-through with minor structural adjustments (e.g. content arrays,
//! `tool_calls` as content parts).

use serde_json::{json, Value};

use super::types::{
    json_opt_f32, json_opt_u32, json_u32, ContentPart, FinishReason, Role, StreamChoice,
    StreamDelta, StreamToolCall, ToolCall, ToolDefinition, ToolResult, UnifiedChoice,
    UnifiedMessage, UnifiedRequest, UnifiedResponse, UnifiedStreamChunk, UnifiedUsage,
};
use super::{ConvertError, FormatConverter};

/// Converter for the `OpenAI` chat-completions format.
#[derive(Debug, Clone, Default)]
pub struct OpenAIConverter;

impl OpenAIConverter {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    fn parse_role(s: &str) -> Result<Role, ConvertError> {
        match s {
            "system" => Ok(Role::System),
            "user" => Ok(Role::User),
            "assistant" => Ok(Role::Assistant),
            "tool" => Ok(Role::Tool),
            other => Err(ConvertError::InvalidRole(other.into())),
        }
    }

    fn role_str(role: Role) -> &'static str {
        match role {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool",
        }
    }

    fn parse_finish_reason(s: &str) -> Option<FinishReason> {
        match s {
            "stop" => Some(FinishReason::Stop),
            "length" => Some(FinishReason::Length),
            "tool_calls" => Some(FinishReason::ToolCalls),
            "content_filter" => Some(FinishReason::ContentFilter),
            _ => None,
        }
    }

    fn finish_reason_str(fr: FinishReason) -> &'static str {
        match fr {
            FinishReason::Stop => "stop",
            FinishReason::Length => "length",
            FinishReason::ToolCalls => "tool_calls",
            FinishReason::ContentFilter => "content_filter",
        }
    }

    fn parse_message(msg: &Value) -> Result<UnifiedMessage, ConvertError> {
        let role_str = msg["role"]
            .as_str()
            .ok_or_else(|| ConvertError::MissingField("role".into()))?;
        let role = Self::parse_role(role_str)?;

        let mut parts = Vec::new();

        // Content can be a string or an array of content parts.
        match &msg["content"] {
            Value::String(s) if !s.is_empty() => {
                parts.push(ContentPart::Text { text: s.clone() });
            }
            Value::Array(arr) => {
                for item in arr {
                    match item["type"].as_str() {
                        Some("text") => {
                            let text = item["text"].as_str().unwrap_or_default().to_string();
                            parts.push(ContentPart::Text { text });
                        }
                        Some("image_url") => {
                            let url = item["image_url"]["url"]
                                .as_str()
                                .unwrap_or_default()
                                .to_string();
                            let detail = item["image_url"]["detail"].as_str().map(String::from);
                            parts.push(ContentPart::ImageUrl { url, detail });
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }

        // Tool calls on assistant messages.
        if let Value::Array(calls) = &msg["tool_calls"] {
            for call in calls {
                let id = call["id"].as_str().unwrap_or_default().to_string();
                let name = call["function"]["name"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let arguments = call["function"]["arguments"]
                    .as_str()
                    .unwrap_or("{}")
                    .to_string();
                parts.push(ContentPart::ToolCall(ToolCall {
                    id,
                    name,
                    arguments,
                }));
            }
        }

        // Tool result messages.
        if role == Role::Tool {
            if let Some(tool_call_id) = msg["tool_call_id"].as_str() {
                let output = msg["content"].as_str().unwrap_or_default().to_string();
                // Clear text parts added above for tool messages; the
                // content IS the tool output.
                parts.retain(|p| !matches!(p, ContentPart::Text { .. }));
                parts.push(ContentPart::ToolResult(ToolResult {
                    tool_call_id: tool_call_id.into(),
                    output,
                }));
            }
        }

        let name = msg["name"].as_str().map(String::from);

        Ok(UnifiedMessage {
            role,
            content: parts,
            name,
        })
    }

    fn message_to_value(msg: &UnifiedMessage) -> Value {
        let mut obj = json!({ "role": Self::role_str(msg.role) });

        if let Some(name) = &msg.name {
            obj["name"] = json!(name);
        }

        // Check for tool result first (role=tool).
        let tool_results: Vec<_> = msg
            .content
            .iter()
            .filter_map(|p| match p {
                ContentPart::ToolResult(tr) => Some(tr),
                _ => None,
            })
            .collect();

        if let Some(tr) = tool_results.first() {
            obj["tool_call_id"] = json!(tr.tool_call_id);
            obj["content"] = json!(tr.output);
            return obj;
        }

        // Collect text + image parts.
        let content_parts: Vec<_> = msg
            .content
            .iter()
            .filter(|p| matches!(p, ContentPart::Text { .. } | ContentPart::ImageUrl { .. }))
            .collect();

        // If only text parts, use simple string.
        let all_text = content_parts
            .iter()
            .all(|p| matches!(p, ContentPart::Text { .. }));

        if content_parts.is_empty() || (all_text && content_parts.len() == 1) {
            let text = msg.text_content();
            obj["content"] = if text.is_empty() {
                Value::Null
            } else {
                json!(text)
            };
        } else {
            let arr: Vec<Value> = content_parts
                .iter()
                .map(|p| match p {
                    ContentPart::Text { text } => json!({"type": "text", "text": text}),
                    ContentPart::ImageUrl { url, detail } => {
                        let mut img = json!({"type": "image_url", "image_url": {"url": url}});
                        if let Some(d) = detail {
                            img["image_url"]["detail"] = json!(d);
                        }
                        img
                    }
                    _ => Value::Null,
                })
                .collect();
            obj["content"] = json!(arr);
        }

        // Tool calls.
        let tool_calls: Vec<_> = msg
            .content
            .iter()
            .filter_map(|p| match p {
                ContentPart::ToolCall(tc) => Some(json!({
                    "id": tc.id,
                    "type": "function",
                    "function": {
                        "name": tc.name,
                        "arguments": tc.arguments,
                    }
                })),
                _ => None,
            })
            .collect();

        if !tool_calls.is_empty() {
            obj["tool_calls"] = json!(tool_calls);
        }

        obj
    }

    fn parse_tools(val: &Value) -> Vec<ToolDefinition> {
        let Some(arr) = val.as_array() else {
            return vec![];
        };
        arr.iter()
            .filter_map(|t| {
                let func = &t["function"];
                Some(ToolDefinition {
                    name: func["name"].as_str()?.to_string(),
                    description: func["description"].as_str().map(String::from),
                    parameters: func["parameters"].clone(),
                })
            })
            .collect()
    }

    fn tools_to_value(tools: &[ToolDefinition]) -> Value {
        let arr: Vec<Value> = tools
            .iter()
            .map(|t| {
                let mut func = json!({
                    "name": t.name,
                    "parameters": t.parameters,
                });
                if let Some(desc) = &t.description {
                    func["description"] = json!(desc);
                }
                json!({"type": "function", "function": func})
            })
            .collect();
        json!(arr)
    }
}

impl FormatConverter for OpenAIConverter {
    fn request_to_unified(&self, body: &Value) -> Result<UnifiedRequest, ConvertError> {
        let model = body["model"].as_str().unwrap_or_default().trim().to_owned();

        let messages = body["messages"]
            .as_array()
            .ok_or_else(|| ConvertError::MissingField("messages".into()))?
            .iter()
            .map(Self::parse_message)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(UnifiedRequest {
            model,
            messages,
            temperature: json_opt_f32(&body["temperature"]),
            max_tokens: json_opt_u32(&body["max_tokens"]),
            top_p: json_opt_f32(&body["top_p"]),
            stop: body["stop"].as_array().map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            }),
            stream: body["stream"].as_bool().unwrap_or(false),
            tools: Self::parse_tools(&body["tools"]),
        })
    }

    fn unified_to_request(&self, req: &UnifiedRequest) -> Result<Value, ConvertError> {
        let messages: Vec<Value> = req.messages.iter().map(Self::message_to_value).collect();

        let mut body = json!({
            "model": req.model,
            "messages": messages,
            "stream": req.stream,
        });

        if let Some(t) = req.temperature {
            body["temperature"] = json!(t);
        }
        if let Some(m) = req.max_tokens {
            body["max_tokens"] = json!(m);
        }
        if let Some(p) = req.top_p {
            body["top_p"] = json!(p);
        }
        if let Some(s) = &req.stop {
            body["stop"] = json!(s);
        }
        if !req.tools.is_empty() {
            body["tools"] = Self::tools_to_value(&req.tools);
        }

        Ok(body)
    }

    fn response_to_unified(&self, body: &Value) -> Result<UnifiedResponse, ConvertError> {
        let id = body["id"].as_str().unwrap_or("").to_string();
        let model = body["model"].as_str().unwrap_or("").to_string();

        let empty = vec![];
        let choices = body["choices"]
            .as_array()
            .unwrap_or(&empty)
            .iter()
            .map(|c| {
                let msg = Self::parse_message(&c["message"])?;
                let finish_reason = c["finish_reason"]
                    .as_str()
                    .and_then(Self::parse_finish_reason);
                Ok(UnifiedChoice {
                    index: json_u32(&c["index"]),
                    message: msg,
                    finish_reason,
                })
            })
            .collect::<Result<Vec<_>, ConvertError>>()?;

        let usage = if body["usage"].is_object() {
            Some(UnifiedUsage {
                prompt_tokens: json_u32(&body["usage"]["prompt_tokens"]),
                completion_tokens: json_u32(&body["usage"]["completion_tokens"]),
                total_tokens: json_u32(&body["usage"]["total_tokens"]),
            })
        } else {
            None
        };

        Ok(UnifiedResponse {
            id,
            model,
            choices,
            usage,
        })
    }

    fn unified_to_response(&self, resp: &UnifiedResponse) -> Result<Value, ConvertError> {
        let choices: Vec<Value> = resp
            .choices
            .iter()
            .map(|c| {
                let mut obj = json!({
                    "index": c.index,
                    "message": Self::message_to_value(&c.message),
                });
                if let Some(fr) = c.finish_reason {
                    obj["finish_reason"] = json!(Self::finish_reason_str(fr));
                } else {
                    obj["finish_reason"] = Value::Null;
                }
                obj
            })
            .collect();

        let mut body = json!({
            "id": resp.id,
            "object": "chat.completion",
            "model": resp.model,
            "choices": choices,
        });

        if let Some(u) = &resp.usage {
            body["usage"] = json!({
                "prompt_tokens": u.prompt_tokens,
                "completion_tokens": u.completion_tokens,
                "total_tokens": u.total_tokens,
            });
        }

        Ok(body)
    }

    fn parse_stream_chunk(&self, data: &str) -> Result<Option<UnifiedStreamChunk>, ConvertError> {
        let trimmed = data.trim();
        if trimmed == "[DONE]" {
            return Ok(None);
        }

        let val: Value = serde_json::from_str(trimmed)?;

        let id = val["id"].as_str().unwrap_or("").to_string();
        let model = val["model"].as_str().unwrap_or("").to_string();

        let empty = vec![];
        let choices = val["choices"]
            .as_array()
            .unwrap_or(&empty)
            .iter()
            .map(|c| {
                let delta = if c["delta"].is_object() {
                    let role = c["delta"]["role"]
                        .as_str()
                        .and_then(|r| Self::parse_role(r).ok());
                    let content = c["delta"]["content"].as_str().map(String::from);
                    let tool_calls = c["delta"]["tool_calls"]
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .map(|tc| StreamToolCall {
                                    index: json_u32(&tc["index"]),
                                    id: tc["id"].as_str().map(String::from),
                                    name: tc["function"]["name"].as_str().map(String::from),
                                    arguments: tc["function"]["arguments"]
                                        .as_str()
                                        .map(String::from),
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    Some(StreamDelta {
                        role,
                        content,
                        tool_calls,
                    })
                } else {
                    None
                };
                let finish_reason = c["finish_reason"]
                    .as_str()
                    .and_then(Self::parse_finish_reason);
                StreamChoice {
                    index: json_u32(&c["index"]),
                    delta,
                    finish_reason,
                }
            })
            .collect();

        let usage = if val["usage"].is_object() {
            Some(UnifiedUsage {
                prompt_tokens: json_u32(&val["usage"]["prompt_tokens"]),
                completion_tokens: json_u32(&val["usage"]["completion_tokens"]),
                total_tokens: json_u32(&val["usage"]["total_tokens"]),
            })
        } else {
            None
        };

        Ok(Some(UnifiedStreamChunk {
            id,
            model,
            choices,
            usage,
        }))
    }

    fn encode_stream_chunk(&self, chunk: &UnifiedStreamChunk) -> Result<String, ConvertError> {
        let choices: Vec<Value> = chunk
            .choices
            .iter()
            .map(|c| {
                let mut obj = json!({ "index": c.index });
                if let Some(delta) = &c.delta {
                    let mut d = json!({});
                    if let Some(role) = delta.role {
                        d["role"] = json!(Self::role_str(role));
                    }
                    if let Some(content) = &delta.content {
                        d["content"] = json!(content);
                    }
                    if !delta.tool_calls.is_empty() {
                        let tcs: Vec<Value> = delta
                            .tool_calls
                            .iter()
                            .map(|tc| {
                                let mut t = json!({"index": tc.index});
                                if let Some(id) = &tc.id {
                                    t["id"] = json!(id);
                                    t["type"] = json!("function");
                                }
                                let mut func = json!({});
                                if let Some(name) = &tc.name {
                                    func["name"] = json!(name);
                                }
                                if let Some(args) = &tc.arguments {
                                    func["arguments"] = json!(args);
                                }
                                t["function"] = func;
                                t
                            })
                            .collect();
                        d["tool_calls"] = json!(tcs);
                    }
                    obj["delta"] = d;
                } else {
                    obj["delta"] = json!({});
                }
                if let Some(fr) = c.finish_reason {
                    obj["finish_reason"] = json!(Self::finish_reason_str(fr));
                } else {
                    obj["finish_reason"] = Value::Null;
                }
                obj
            })
            .collect();

        let mut body = json!({
            "id": chunk.id,
            "object": "chat.completion.chunk",
            "model": chunk.model,
            "choices": choices,
        });

        if let Some(u) = &chunk.usage {
            body["usage"] = json!({
                "prompt_tokens": u.prompt_tokens,
                "completion_tokens": u.completion_tokens,
                "total_tokens": u.total_tokens,
            });
        }

        let line = serde_json::to_string(&body)?;
        Ok(format!("data: {line}\n\n"))
    }

    fn encode_stream_done(&self) -> String {
        "data: [DONE]\n\n".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn converter() -> OpenAIConverter {
        OpenAIConverter::new()
    }

    #[test]
    fn simple_request_roundtrip() {
        let input = json!({
            "model": "gpt-5.4",
            "messages": [{"role": "user", "content": "hello"}],
            "temperature": 0.7,
            "max_tokens": 100,
            "stream": false
        });
        let unified = converter().request_to_unified(&input).unwrap();
        assert_eq!(unified.model, "gpt-5.4");
        assert_eq!(unified.messages.len(), 1);
        assert_eq!(unified.messages[0].text_content(), "hello");

        let back = converter().unified_to_request(&unified).unwrap();
        assert_eq!(back["model"], "gpt-5.4");
        assert_eq!(back["messages"][0]["content"], "hello");
    }

    #[test]
    fn multimodal_content_array() {
        let input = json!({
            "model": "gpt-5.4",
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "text", "text": "describe"},
                    {"type": "image_url", "image_url": {"url": "https://img.png", "detail": "high"}}
                ]
            }],
            "stream": false
        });
        let unified = converter().request_to_unified(&input).unwrap();
        assert_eq!(unified.messages[0].content.len(), 2);
        assert!(
            matches!(&unified.messages[0].content[1], ContentPart::ImageUrl { url, .. } if url == "https://img.png")
        );
    }

    #[test]
    fn tool_calls_roundtrip() {
        let input = json!({
            "model": "gpt-5.4",
            "messages": [
                {"role": "user", "content": "weather?"},
                {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_abc",
                        "type": "function",
                        "function": {
                            "name": "get_weather",
                            "arguments": "{\"location\":\"Tokyo\"}"
                        }
                    }]
                },
                {
                    "role": "tool",
                    "tool_call_id": "call_abc",
                    "content": "sunny, 25C"
                }
            ],
            "stream": false
        });

        let unified = converter().request_to_unified(&input).unwrap();
        assert_eq!(unified.messages.len(), 3);

        let tc = unified.messages[1].tool_calls();
        assert_eq!(tc.len(), 1);
        assert_eq!(tc[0].name, "get_weather");

        assert!(matches!(
            &unified.messages[2].content[0],
            ContentPart::ToolResult(tr) if tr.output == "sunny, 25C"
        ));

        let back = converter().unified_to_request(&unified).unwrap();
        assert_eq!(
            back["messages"][1]["tool_calls"][0]["function"]["name"],
            "get_weather"
        );
        assert_eq!(back["messages"][2]["tool_call_id"], "call_abc");
    }

    #[test]
    fn response_roundtrip() {
        let input = json!({
            "id": "chatcmpl-abc",
            "object": "chat.completion",
            "model": "gpt-5.4",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "Hi!"},
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15
            }
        });
        let unified = converter().response_to_unified(&input).unwrap();
        assert_eq!(unified.id, "chatcmpl-abc");
        assert_eq!(unified.choices[0].finish_reason, Some(FinishReason::Stop));
        assert_eq!(unified.usage.as_ref().unwrap().total_tokens, 15);

        let back = converter().unified_to_response(&unified).unwrap();
        assert_eq!(back["id"], "chatcmpl-abc");
        assert_eq!(back["object"], "chat.completion");
    }

    #[test]
    fn tool_definitions_roundtrip() {
        let input = json!({
            "model": "gpt-5.4",
            "messages": [{"role": "user", "content": "hi"}],
            "tools": [{
                "type": "function",
                "function": {
                    "name": "get_weather",
                    "description": "Get weather",
                    "parameters": {"type": "object", "properties": {"loc": {"type": "string"}}}
                }
            }],
            "stream": false
        });
        let unified = converter().request_to_unified(&input).unwrap();
        assert_eq!(unified.tools.len(), 1);
        assert_eq!(unified.tools[0].name, "get_weather");

        let back = converter().unified_to_request(&unified).unwrap();
        assert_eq!(back["tools"][0]["function"]["name"], "get_weather");
    }

    #[test]
    fn stream_chunk_parse_and_encode() {
        let data = r#"{"id":"chatcmpl-1","object":"chat.completion.chunk","model":"gpt-5.4","choices":[{"index":0,"delta":{"role":"assistant","content":"Hi"},"finish_reason":null}]}"#;
        let chunk = converter().parse_stream_chunk(data).unwrap().unwrap();
        assert_eq!(chunk.id, "chatcmpl-1");
        assert_eq!(
            chunk.choices[0].delta.as_ref().unwrap().content.as_deref(),
            Some("Hi")
        );

        let encoded = converter().encode_stream_chunk(&chunk).unwrap();
        assert!(encoded.starts_with("data: "));
        assert!(encoded.ends_with("\n\n"));
    }

    #[test]
    fn stream_done_signal() {
        let result = converter().parse_stream_chunk("[DONE]").unwrap();
        assert!(result.is_none());

        let done = converter().encode_stream_done();
        assert_eq!(done, "data: [DONE]\n\n");
    }

    #[test]
    fn stream_tool_call_delta() {
        let data = json!({
            "id": "chatcmpl-2",
            "model": "gpt-5.4",
            "choices": [{
                "index": 0,
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "id": "call_1",
                        "type": "function",
                        "function": {"name": "search", "arguments": "{\"q\":"}
                    }]
                },
                "finish_reason": null
            }]
        });
        let chunk = converter()
            .parse_stream_chunk(&data.to_string())
            .unwrap()
            .unwrap();
        let tc = &chunk.choices[0].delta.as_ref().unwrap().tool_calls;
        assert_eq!(tc.len(), 1);
        assert_eq!(tc[0].name.as_deref(), Some("search"));
    }

    #[test]
    fn missing_model_defaults_to_empty_for_provider_fallback() {
        let input = json!({"messages": [{"role": "user", "content": "hi"}]});
        let unified = converter().request_to_unified(&input).unwrap();

        assert_eq!(unified.model, "");
    }
}
