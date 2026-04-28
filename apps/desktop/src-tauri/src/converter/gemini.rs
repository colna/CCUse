//! Google Gemini API format converter (T1.0.3.05 + T1.0.3.10).
//!
//! Key differences from `OpenAI`:
//! - Request uses `contents` (not `messages`), each with `parts`
//! - Roles: `user` and `model` (not `assistant`); `system` uses
//!   `system_instruction` top-level field
//! - Tool calls use `functionCall` / `functionResponse` parts
//! - Tool definitions use `functionDeclarations` inside a single
//!   `tools` array entry
//! - SSE: `data: {"candidates":[{"content":{"parts":[...]}}]}`

use serde_json::{json, Value};

use super::types::{
    json_opt_f32, json_opt_u32, json_u32, ContentPart, FinishReason, Role, StreamChoice,
    StreamDelta, StreamToolCall, ToolCall, ToolDefinition, ToolResult, UnifiedChoice,
    UnifiedMessage, UnifiedRequest, UnifiedResponse, UnifiedStreamChunk, UnifiedUsage,
};
use super::{ConvertError, FormatConverter};

/// Converter for the Google Gemini `generateContent` format.
#[derive(Debug, Clone, Default)]
pub struct GeminiConverter;

impl GeminiConverter {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    fn parse_role(s: &str) -> Result<Role, ConvertError> {
        match s {
            "user" => Ok(Role::User),
            "model" => Ok(Role::Assistant),
            other => Err(ConvertError::InvalidRole(other.into())),
        }
    }

    fn role_str(role: Role) -> &'static str {
        match role {
            Role::User | Role::System | Role::Tool => "user",
            Role::Assistant => "model",
        }
    }

    fn parse_finish_reason(s: &str) -> Option<FinishReason> {
        match s {
            "STOP" => Some(FinishReason::Stop),
            "MAX_TOKENS" => Some(FinishReason::Length),
            "SAFETY" => Some(FinishReason::ContentFilter),
            _ => None,
        }
    }

    fn finish_reason_str(fr: FinishReason) -> &'static str {
        match fr {
            FinishReason::Length => "MAX_TOKENS",
            FinishReason::ContentFilter => "SAFETY",
            FinishReason::Stop | FinishReason::ToolCalls => "STOP",
        }
    }

    fn parse_parts(parts: &[Value]) -> Vec<ContentPart> {
        let mut result = Vec::new();
        for part in parts {
            if let Some(text) = part["text"].as_str() {
                result.push(ContentPart::Text {
                    text: text.to_string(),
                });
            } else if part["inlineData"].is_object() {
                let mime = part["inlineData"]["mimeType"]
                    .as_str()
                    .unwrap_or("image/png");
                let data = part["inlineData"]["data"]
                    .as_str()
                    .unwrap_or_default();
                let url = format!("data:{mime};base64,{data}");
                result.push(ContentPart::ImageUrl { url, detail: None });
            } else if part["functionCall"].is_object() {
                let name = part["functionCall"]["name"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let args = serde_json::to_string(&part["functionCall"]["args"])
                    .unwrap_or_else(|_| "{}".into());
                let id = format!("gemini_call_{name}");
                result.push(ContentPart::ToolCall(ToolCall {
                    id,
                    name,
                    arguments: args,
                }));
            } else if part["functionResponse"].is_object() {
                let name = part["functionResponse"]["name"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let response = serde_json::to_string(
                    &part["functionResponse"]["response"],
                )
                .unwrap_or_else(|_| "{}".into());
                let tool_call_id = format!("gemini_call_{name}");
                result.push(ContentPart::ToolResult(ToolResult {
                    tool_call_id,
                    output: response,
                }));
            }
        }
        result
    }

    fn parts_to_value(parts: &[ContentPart]) -> Vec<Value> {
        parts
            .iter()
            .map(|p| match p {
                ContentPart::Text { text } => json!({"text": text}),
                ContentPart::ImageUrl { url, .. } => {
                    if let Some(rest) = url.strip_prefix("data:") {
                        if let Some((mime, data)) = rest.split_once(";base64,") {
                            return json!({
                                "inlineData": { "mimeType": mime, "data": data }
                            });
                        }
                    }
                    json!({"text": format!("[image: {url}]")})
                }
                ContentPart::ToolCall(tc) => {
                    let args: Value =
                        serde_json::from_str(&tc.arguments).unwrap_or(json!({}));
                    json!({ "functionCall": { "name": tc.name, "args": args } })
                }
                ContentPart::ToolResult(tr) => {
                    let name = tr
                        .tool_call_id
                        .strip_prefix("gemini_call_")
                        .unwrap_or(&tr.tool_call_id);
                    let response: Value =
                        serde_json::from_str(&tr.output).unwrap_or(json!({"result": tr.output}));
                    json!({ "functionResponse": { "name": name, "response": response } })
                }
            })
            .collect()
    }

    fn parse_usage_metadata(val: &Value) -> Option<UnifiedUsage> {
        if !val["usageMetadata"].is_object() {
            return None;
        }
        let um = &val["usageMetadata"];
        Some(UnifiedUsage {
            prompt_tokens: json_u32(&um["promptTokenCount"]),
            completion_tokens: json_u32(&um["candidatesTokenCount"]),
            total_tokens: json_u32(&um["totalTokenCount"]),
        })
    }
}

impl FormatConverter for GeminiConverter {
    fn request_to_unified(&self, body: &Value) -> Result<UnifiedRequest, ConvertError> {
        let mut messages = Vec::new();

        // System instruction.
        if let Some(sys) = body["system_instruction"]["parts"].as_array() {
            let text = sys
                .iter()
                .filter_map(|p| p["text"].as_str())
                .collect::<Vec<_>>()
                .join("");
            if !text.is_empty() {
                messages.push(UnifiedMessage::text(Role::System, text));
            }
        }

        let contents = body["contents"]
            .as_array()
            .ok_or_else(|| ConvertError::MissingField("contents".into()))?;

        for content in contents {
            let role_str = content["role"]
                .as_str()
                .ok_or_else(|| ConvertError::MissingField("role".into()))?;
            let role = Self::parse_role(role_str)?;
            let parts_arr = content["parts"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let parts = Self::parse_parts(&parts_arr);
            messages.push(UnifiedMessage {
                role,
                content: parts,
                name: None,
            });
        }

        let model = body["model"]
            .as_str()
            .unwrap_or("gemini-1.5-pro")
            .to_string();

        let gen_config = &body["generationConfig"];

        let tools = body["tools"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|t| t["functionDeclarations"].as_array())
            .map(|decls| {
                decls
                    .iter()
                    .filter_map(|d| {
                        Some(ToolDefinition {
                            name: d["name"].as_str()?.to_string(),
                            description: d["description"].as_str().map(String::from),
                            parameters: d["parameters"].clone(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(UnifiedRequest {
            model,
            messages,
            temperature: json_opt_f32(&gen_config["temperature"]),
            max_tokens: json_opt_u32(&gen_config["maxOutputTokens"]),
            top_p: json_opt_f32(&gen_config["topP"]),
            stop: gen_config["stopSequences"].as_array().map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            }),
            stream: false,
            tools,
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

        let contents: Vec<Value> = req
            .messages
            .iter()
            .filter(|m| m.role != Role::System)
            .map(|m| {
                json!({
                    "role": Self::role_str(m.role),
                    "parts": Self::parts_to_value(&m.content),
                })
            })
            .collect();

        let mut body = json!({ "contents": contents });

        if !system_text.is_empty() {
            body["system_instruction"] = json!({
                "parts": [{"text": system_text}]
            });
        }

        let mut gen_config = json!({});
        if let Some(t) = req.temperature {
            gen_config["temperature"] = json!(t);
        }
        if let Some(m) = req.max_tokens {
            gen_config["maxOutputTokens"] = json!(m);
        }
        if let Some(p) = req.top_p {
            gen_config["topP"] = json!(p);
        }
        if let Some(s) = &req.stop {
            gen_config["stopSequences"] = json!(s);
        }
        if gen_config != json!({}) {
            body["generationConfig"] = gen_config;
        }

        if !req.tools.is_empty() {
            let decls: Vec<Value> = req
                .tools
                .iter()
                .map(|t| {
                    let mut decl = json!({
                        "name": t.name,
                        "parameters": t.parameters,
                    });
                    if let Some(desc) = &t.description {
                        decl["description"] = json!(desc);
                    }
                    decl
                })
                .collect();
            body["tools"] = json!([{"functionDeclarations": decls}]);
        }

        Ok(body)
    }

    fn response_to_unified(&self, body: &Value) -> Result<UnifiedResponse, ConvertError> {
        let candidates = body["candidates"]
            .as_array()
            .cloned()
            .unwrap_or_default();

        let choices = candidates
            .iter()
            .enumerate()
            .map(|(i, cand)| {
                let parts_arr = cand["content"]["parts"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                let parts = Self::parse_parts(&parts_arr);
                let role = cand["content"]["role"]
                    .as_str()
                    .and_then(|r| Self::parse_role(r).ok())
                    .unwrap_or(Role::Assistant);

                let finish_reason = cand["finishReason"]
                    .as_str()
                    .and_then(Self::parse_finish_reason);

                Ok(UnifiedChoice {
                    index: u32::try_from(i).unwrap_or(0),
                    message: UnifiedMessage {
                        role,
                        content: parts,
                        name: None,
                    },
                    finish_reason,
                })
            })
            .collect::<Result<Vec<_>, ConvertError>>()?;

        let model = body["modelVersion"]
            .as_str()
            .unwrap_or("gemini")
            .to_string();

        Ok(UnifiedResponse {
            id: format!("gemini-{}", uuid::Uuid::new_v4()),
            model,
            choices,
            usage: Self::parse_usage_metadata(body),
        })
    }

    fn unified_to_response(&self, resp: &UnifiedResponse) -> Result<Value, ConvertError> {
        let candidates: Vec<Value> = resp
            .choices
            .iter()
            .map(|c| {
                let parts = Self::parts_to_value(&c.message.content);
                let mut cand = json!({
                    "content": {
                        "role": Self::role_str(c.message.role),
                        "parts": parts,
                    }
                });
                if let Some(fr) = c.finish_reason {
                    cand["finishReason"] = json!(Self::finish_reason_str(fr));
                }
                cand
            })
            .collect();

        let mut body = json!({ "candidates": candidates });

        if let Some(u) = &resp.usage {
            body["usageMetadata"] = json!({
                "promptTokenCount": u.prompt_tokens,
                "candidatesTokenCount": u.completion_tokens,
                "totalTokenCount": u.total_tokens,
            });
        }

        Ok(body)
    }

    fn parse_stream_chunk(&self, data: &str) -> Result<Option<UnifiedStreamChunk>, ConvertError> {
        let trimmed = data.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }

        let val: Value = serde_json::from_str(trimmed)?;

        let candidates = val["candidates"]
            .as_array()
            .cloned()
            .unwrap_or_default();

        if candidates.is_empty() {
            return if val["usageMetadata"].is_object() {
                Ok(Some(UnifiedStreamChunk {
                    id: String::new(),
                    model: String::new(),
                    choices: vec![],
                    usage: Self::parse_usage_metadata(&val),
                }))
            } else {
                Ok(None)
            };
        }

        let choices = candidates
            .iter()
            .enumerate()
            .map(|(i, cand)| {
                let parts = cand["content"]["parts"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();

                let mut content_text = None;
                let mut tool_calls = Vec::new();

                for (pi, part) in parts.iter().enumerate() {
                    if let Some(text) = part["text"].as_str() {
                        content_text = Some(text.to_string());
                    }
                    if part["functionCall"].is_object() {
                        let name = part["functionCall"]["name"]
                            .as_str()
                            .unwrap_or_default()
                            .to_string();
                        let args = serde_json::to_string(&part["functionCall"]["args"])
                            .unwrap_or_else(|_| "{}".into());
                        tool_calls.push(StreamToolCall {
                            index: u32::try_from(pi).unwrap_or(0),
                            id: Some(format!("gemini_call_{name}")),
                            name: Some(name),
                            arguments: Some(args),
                        });
                    }
                }

                let finish_reason = cand["finishReason"]
                    .as_str()
                    .and_then(Self::parse_finish_reason);

                StreamChoice {
                    index: u32::try_from(i).unwrap_or(0),
                    delta: Some(StreamDelta {
                        role: None,
                        content: content_text,
                        tool_calls,
                    }),
                    finish_reason,
                }
            })
            .collect();

        Ok(Some(UnifiedStreamChunk {
            id: String::new(),
            model: val["modelVersion"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            choices,
            usage: Self::parse_usage_metadata(&val),
        }))
    }

    fn encode_stream_chunk(&self, chunk: &UnifiedStreamChunk) -> Result<String, ConvertError> {
        let candidates: Vec<Value> = chunk
            .choices
            .iter()
            .map(|c| {
                let mut parts = Vec::new();
                if let Some(delta) = &c.delta {
                    if let Some(content) = &delta.content {
                        parts.push(json!({"text": content}));
                    }
                    for tc in &delta.tool_calls {
                        if let (Some(name), Some(args)) = (&tc.name, &tc.arguments) {
                            let args_val: Value =
                                serde_json::from_str(args).unwrap_or(json!({}));
                            parts.push(json!({
                                "functionCall": { "name": name, "args": args_val }
                            }));
                        }
                    }
                }
                let mut cand = json!({
                    "content": { "role": "model", "parts": parts }
                });
                if let Some(fr) = c.finish_reason {
                    cand["finishReason"] = json!(Self::finish_reason_str(fr));
                }
                cand
            })
            .collect();

        let mut body = json!({"candidates": candidates});

        if let Some(u) = &chunk.usage {
            body["usageMetadata"] = json!({
                "promptTokenCount": u.prompt_tokens,
                "candidatesTokenCount": u.completion_tokens,
                "totalTokenCount": u.total_tokens,
            });
        }

        let line = serde_json::to_string(&body)?;
        Ok(format!("data: {line}\n\n"))
    }

    fn encode_stream_done(&self) -> String {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn converter() -> GeminiConverter {
        GeminiConverter::new()
    }

    #[test]
    fn simple_request_roundtrip() {
        let input = json!({
            "contents": [
                {"role": "user", "parts": [{"text": "hello"}]}
            ],
            "system_instruction": {"parts": [{"text": "be helpful"}]},
            "generationConfig": {"temperature": 0.7, "maxOutputTokens": 100}
        });
        let unified = converter().request_to_unified(&input).unwrap();
        assert_eq!(unified.messages.len(), 2);
        assert_eq!(unified.messages[0].role, Role::System);
        assert_eq!(unified.messages[0].text_content(), "be helpful");
        assert_eq!(unified.messages[1].text_content(), "hello");
        assert!((unified.temperature.unwrap() - 0.7).abs() < f32::EPSILON);

        let back = converter().unified_to_request(&unified).unwrap();
        assert_eq!(back["system_instruction"]["parts"][0]["text"], "be helpful");
        assert_eq!(back["contents"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn function_call_roundtrip() {
        let input = json!({
            "contents": [
                {"role": "user", "parts": [{"text": "weather?"}]},
                {"role": "model", "parts": [{
                    "functionCall": {"name": "get_weather", "args": {"location": "Tokyo"}}
                }]},
                {"role": "user", "parts": [{
                    "functionResponse": {"name": "get_weather", "response": {"temp": "25C"}}
                }]}
            ],
            "tools": [{
                "functionDeclarations": [{
                    "name": "get_weather",
                    "description": "Get weather",
                    "parameters": {"type": "object", "properties": {"location": {"type": "string"}}}
                }]
            }]
        });
        let unified = converter().request_to_unified(&input).unwrap();
        assert_eq!(unified.messages.len(), 3);

        let tc = unified.messages[1].tool_calls();
        assert_eq!(tc.len(), 1);
        assert_eq!(tc[0].name, "get_weather");

        assert!(matches!(
            &unified.messages[2].content[0],
            ContentPart::ToolResult(tr) if tr.tool_call_id == "gemini_call_get_weather"
        ));

        assert_eq!(unified.tools.len(), 1);

        let back = converter().unified_to_request(&unified).unwrap();
        assert!(back["contents"][1]["parts"][0]["functionCall"].is_object());
    }

    #[test]
    fn response_parsing() {
        let input = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{"text": "Hi there!"}]
                },
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 5,
                "totalTokenCount": 15
            }
        });
        let unified = converter().response_to_unified(&input).unwrap();
        assert_eq!(unified.choices.len(), 1);
        assert_eq!(unified.choices[0].message.text_content(), "Hi there!");
        assert_eq!(unified.choices[0].finish_reason, Some(FinishReason::Stop));
        assert_eq!(unified.usage.as_ref().unwrap().total_tokens, 15);
    }

    #[test]
    fn response_roundtrip() {
        let resp = UnifiedResponse {
            id: "test".into(),
            model: "gemini-1.5-pro".into(),
            choices: vec![UnifiedChoice {
                index: 0,
                message: UnifiedMessage::text(Role::Assistant, "Hello"),
                finish_reason: Some(FinishReason::Stop),
            }],
            usage: Some(UnifiedUsage {
                prompt_tokens: 5,
                completion_tokens: 2,
                total_tokens: 7,
            }),
        };
        let val = converter().unified_to_response(&resp).unwrap();
        assert_eq!(val["candidates"][0]["finishReason"], "STOP");
        assert_eq!(val["candidates"][0]["content"]["role"], "model");
    }

    #[test]
    fn stream_text_chunk() {
        let data = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{"text": "Hello"}]
                }
            }]
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
    fn stream_function_call_chunk() {
        let data = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{"functionCall": {"name": "search", "args": {"q": "rust"}}}]
                }
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
    fn empty_stream_data() {
        let result = converter().parse_stream_chunk("").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn inline_data_image() {
        let input = json!({
            "contents": [{
                "role": "user",
                "parts": [{
                    "inlineData": {"mimeType": "image/jpeg", "data": "abc123"}
                }]
            }]
        });
        let unified = converter().request_to_unified(&input).unwrap();
        assert!(matches!(
            &unified.messages[0].content[0],
            ContentPart::ImageUrl { url, .. } if url == "data:image/jpeg;base64,abc123"
        ));
    }
}
