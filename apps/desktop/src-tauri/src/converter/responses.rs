//! `OpenAI` Responses API (`POST /v1/responses`) request parser (T1.0.6.36).
//!
//! Responses is the newer agentic surface that ships with the
//! `responses.create()` SDK call, Codex CLI, and several IDE
//! integrations. Its request shape is a superset of chat-completions
//! with a richer `input` schema (string / array of typed items),
//! `instructions` as a top-level system override, a flatter `tools`
//! array, and stateful conversation fields we deliberately do not yet
//! implement.
//!
//! This converter only exposes the request side for now. Response
//! serialisation and streaming live in T1.0.6.37 / T1.0.6.38.

use serde_json::Value;

use super::types::{
    ContentPart, Role, ToolCall, ToolDefinition, ToolResult, UnifiedMessage, UnifiedRequest,
};
use super::ConvertError;

/// Converter for `OpenAI` Responses (`/v1/responses`) inbound requests.
///
/// The struct is a ZST — keep it like [`OpenAIConverter`] so we can
/// hand it to handlers via [`ProxyAppState`] at zero runtime cost.
///
/// [`OpenAIConverter`]: super::OpenAIConverter
/// [`ProxyAppState`]: crate::proxy::ProxyAppState
#[derive(Debug, Clone, Copy, Default)]
pub struct ResponsesConverter;

impl ResponsesConverter {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Parse a Responses-shaped request body into [`UnifiedRequest`].
    ///
    /// Behaviour deliberately mirrors what real clients send:
    /// - `input` accepts a string, an array of strings, or an array of
    ///   typed items (`message`, `function_call`, `function_call_output`).
    /// - `instructions` becomes a prepended `system` message — and wins
    ///   over a `system` role inside `input` per the Responses spec.
    /// - `tools` only accepts `type = "function"` for now; any builtin
    ///   tool (`web_search`, `file_search`, `code_interpreter`,
    ///   `image_generation`, `mcp`, `computer_use`) is rejected with
    ///   [`ConvertError::UnsupportedContent`].
    /// - `previous_response_id`, `store`, `text`, `reasoning`,
    ///   `metadata` are silently ignored — stateless proxy plus
    ///   forward-compatibility.
    pub fn request_to_unified(&self, body: &Value) -> Result<UnifiedRequest, ConvertError> {
        let model = body
            .get("model")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ConvertError::MissingField("model".into()))?
            .to_owned();

        let input = body
            .get("input")
            .ok_or_else(|| ConvertError::MissingField("input".into()))?;

        let mut messages = parse_input(input)?;

        if let Some(instructions) = body.get("instructions").and_then(Value::as_str) {
            if !instructions.is_empty() {
                messages.insert(
                    0,
                    UnifiedMessage {
                        role: Role::System,
                        content: vec![ContentPart::Text {
                            text: instructions.to_owned(),
                        }],
                        name: None,
                    },
                );
            }
        }

        let tools = match body.get("tools") {
            Some(Value::Array(arr)) => parse_tools(arr)?,
            _ => Vec::new(),
        };

        // OpenAI clients always send f64; chat-completions semantics
        // downstream want f32. Truncation here is intentional and bounded
        // by the API's documented [0.0, 2.0] range.
        #[expect(
            clippy::cast_possible_truncation,
            reason = "temperature/top_p narrow on purpose"
        )]
        let temperature = body
            .get("temperature")
            .and_then(Value::as_f64)
            .map(|v| v as f32);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "temperature/top_p narrow on purpose"
        )]
        let top_p = body.get("top_p").and_then(Value::as_f64).map(|v| v as f32);
        // Responses calls it `max_output_tokens`; we keep `max_tokens`
        // on the unified side because downstream providers all speak
        // chat-completions semantics.
        let max_tokens = body
            .get("max_output_tokens")
            .or_else(|| body.get("max_tokens"))
            .and_then(Value::as_u64)
            .and_then(|v| u32::try_from(v).ok());
        let stop = body
            .get("stop")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect::<Vec<_>>()
            })
            .filter(|v| !v.is_empty());
        let stream = body.get("stream").and_then(Value::as_bool).unwrap_or(false);

        Ok(UnifiedRequest {
            model,
            messages,
            temperature,
            max_tokens,
            top_p,
            stop,
            stream,
            tools,
        })
    }
}

/// Translate the `input` field into unified messages.
fn parse_input(input: &Value) -> Result<Vec<UnifiedMessage>, ConvertError> {
    match input {
        Value::String(text) => Ok(vec![UnifiedMessage {
            role: Role::User,
            content: vec![ContentPart::Text { text: text.clone() }],
            name: None,
        }]),
        Value::Array(items) => {
            let mut messages: Vec<UnifiedMessage> = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    Value::String(text) => {
                        messages.push(UnifiedMessage {
                            role: Role::User,
                            content: vec![ContentPart::Text { text: text.clone() }],
                            name: None,
                        });
                    }
                    Value::Object(_) => {
                        parse_input_item(item, &mut messages)?;
                    }
                    _ => {
                        return Err(ConvertError::UnsupportedContent(format!(
                            "input array element: {item}"
                        )));
                    }
                }
            }
            Ok(messages)
        }
        _ => Err(ConvertError::UnsupportedContent(
            "input must be a string or an array".into(),
        )),
    }
}

/// Dispatch a single object inside the `input` array to the matching
/// item-shape parser. Function-call items attach to the previous
/// assistant message instead of creating a new one — that's what
/// `OpenAI` clients send back when echoing a conversation.
fn parse_input_item(item: &Value, messages: &mut Vec<UnifiedMessage>) -> Result<(), ConvertError> {
    // Plain chat-completions shape: `{ role, content }` without a `type`.
    if item.get("type").is_none() {
        if let Some(role_str) = item.get("role").and_then(Value::as_str) {
            messages.push(parse_chat_style_message(role_str, item)?);
            return Ok(());
        }
        return Err(ConvertError::UnsupportedContent(format!(
            "input item missing both `type` and `role`: {item}"
        )));
    }

    let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");
    match item_type {
        "message" => {
            let role_str = item
                .get("role")
                .and_then(Value::as_str)
                .ok_or_else(|| ConvertError::MissingField("input[].role".into()))?;
            messages.push(parse_responses_style_message(role_str, item)?);
            Ok(())
        }
        "function_call" => {
            let call = ToolCall {
                id: item
                    .get("call_id")
                    .and_then(Value::as_str)
                    .or_else(|| item.get("id").and_then(Value::as_str))
                    .unwrap_or_default()
                    .to_owned(),
                name: item
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                arguments: item
                    .get("arguments")
                    .and_then(Value::as_str)
                    .unwrap_or("{}")
                    .to_owned(),
            };
            // Attach to the trailing assistant message if there is one;
            // otherwise wrap it in a fresh assistant message so the
            // tool call still surfaces to the provider.
            if let Some(last) = messages
                .last_mut()
                .filter(|m| matches!(m.role, Role::Assistant))
            {
                last.content.push(ContentPart::ToolCall(call));
            } else {
                messages.push(UnifiedMessage {
                    role: Role::Assistant,
                    content: vec![ContentPart::ToolCall(call)],
                    name: None,
                });
            }
            Ok(())
        }
        "function_call_output" => {
            let result = ToolResult {
                tool_call_id: item
                    .get("call_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                output: item
                    .get("output")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            };
            messages.push(UnifiedMessage {
                role: Role::Tool,
                content: vec![ContentPart::ToolResult(result)],
                name: None,
            });
            Ok(())
        }
        other => Err(ConvertError::UnsupportedContent(format!(
            "input item type `{other}` is not supported"
        ))),
    }
}

/// Chat-completions style message: `{ role, content }`. `content`
/// may be a plain string or an array of parts using
/// `{type:"text", text}` / `{type:"image_url", image_url}`.
fn parse_chat_style_message(role_str: &str, item: &Value) -> Result<UnifiedMessage, ConvertError> {
    let role = parse_role(role_str)?;
    let parts = match item.get("content") {
        Some(Value::String(s)) if !s.is_empty() => vec![ContentPart::Text { text: s.clone() }],
        Some(Value::Array(arr)) => parse_chat_content_array(arr),
        _ => Vec::new(),
    };
    Ok(UnifiedMessage {
        role,
        content: parts,
        name: None,
    })
}

fn parse_chat_content_array(arr: &[Value]) -> Vec<ContentPart> {
    let mut parts = Vec::with_capacity(arr.len());
    for item in arr {
        match item.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(text) = item.get("text").and_then(Value::as_str) {
                    parts.push(ContentPart::Text {
                        text: text.to_owned(),
                    });
                }
            }
            Some("image_url") => {
                let url = item
                    .pointer("/image_url/url")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                let detail = item
                    .pointer("/image_url/detail")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                parts.push(ContentPart::ImageUrl { url, detail });
            }
            _ => {}
        }
    }
    parts
}

/// Responses-style message item: `{ type: "message", role, content: [...] }`
/// where each content entry uses `input_text` / `input_image` /
/// `output_text`.
fn parse_responses_style_message(
    role_str: &str,
    item: &Value,
) -> Result<UnifiedMessage, ConvertError> {
    let role = parse_role(role_str)?;
    let parts = match item.get("content") {
        Some(Value::String(s)) if !s.is_empty() => vec![ContentPart::Text { text: s.clone() }],
        Some(Value::Array(arr)) => parse_responses_content_array(arr),
        _ => Vec::new(),
    };
    Ok(UnifiedMessage {
        role,
        content: parts,
        name: None,
    })
}

fn parse_responses_content_array(arr: &[Value]) -> Vec<ContentPart> {
    let mut parts = Vec::with_capacity(arr.len());
    for item in arr {
        match item.get("type").and_then(Value::as_str) {
            Some("input_text" | "output_text" | "text") => {
                if let Some(text) = item.get("text").and_then(Value::as_str) {
                    parts.push(ContentPart::Text {
                        text: text.to_owned(),
                    });
                }
            }
            Some("input_image" | "image_url") => {
                // Real OpenAI clients vary between `image_url: "<url>"`
                // (a string) and `image_url: { url, detail }` (an
                // object). Accept both so we don't reject valid input.
                let url = item
                    .get("image_url")
                    .and_then(|v| {
                        v.as_str().map(str::to_owned).or_else(|| {
                            v.pointer("/url").and_then(Value::as_str).map(str::to_owned)
                        })
                    })
                    .unwrap_or_default();
                let detail = item
                    .pointer("/image_url/detail")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                parts.push(ContentPart::ImageUrl { url, detail });
            }
            _ => {}
        }
    }
    parts
}

fn parse_role(s: &str) -> Result<Role, ConvertError> {
    match s {
        "system" | "developer" => Ok(Role::System),
        "user" => Ok(Role::User),
        "assistant" => Ok(Role::Assistant),
        "tool" => Ok(Role::Tool),
        other => Err(ConvertError::InvalidRole(other.into())),
    }
}

/// Responses uses a flat tool shape:
/// `{ type: "function", name, description?, parameters }`.
/// Chat-completions nests it under `function`. Any non-function `type`
/// is a builtin (`web_search`, `file_search`, `code_interpreter`,
/// `image_generation`, `mcp`, `computer_use`) — we reject those
/// rather than silently dropping the tool, since the model would then
/// quietly produce wrong output.
fn parse_tools(arr: &[Value]) -> Result<Vec<ToolDefinition>, ConvertError> {
    let mut tools = Vec::with_capacity(arr.len());
    for item in arr {
        let tool_type = item
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("function");
        if tool_type != "function" {
            return Err(ConvertError::UnsupportedContent(format!(
                "builtin tool: {tool_type}"
            )));
        }
        let name = item
            .get("name")
            .or_else(|| item.pointer("/function/name"))
            .and_then(Value::as_str)
            .ok_or_else(|| ConvertError::MissingField("tools[].name".into()))?
            .to_owned();
        let description = item
            .get("description")
            .or_else(|| item.pointer("/function/description"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        let parameters = item
            .get("parameters")
            .or_else(|| item.pointer("/function/parameters"))
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        tools.push(ToolDefinition {
            name,
            description,
            parameters,
        });
    }
    Ok(tools)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn convert(body: &serde_json::Value) -> Result<UnifiedRequest, ConvertError> {
        ResponsesConverter.request_to_unified(body)
    }

    #[test]
    fn string_input_becomes_single_user_text_message() {
        let req = convert(&json!({"model": "gpt-4o", "input": "hello"})).unwrap();
        assert_eq!(req.model, "gpt-4o");
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.messages[0].role, Role::User);
        assert_eq!(
            req.messages[0].content,
            vec![ContentPart::Text {
                text: "hello".into()
            }]
        );
    }

    #[test]
    fn array_of_strings_becomes_multiple_user_messages() {
        let req = convert(&json!({
            "model": "gpt-4o",
            "input": ["first", "second"],
        }))
        .unwrap();
        assert_eq!(req.messages.len(), 2);
        for msg in &req.messages {
            assert_eq!(msg.role, Role::User);
        }
    }

    #[test]
    fn chat_style_user_assistant_history_is_preserved() {
        let req = convert(&json!({
            "model": "gpt-4o",
            "input": [
                {"role": "user", "content": "ping"},
                {"role": "assistant", "content": "pong"},
            ],
        }))
        .unwrap();
        assert_eq!(req.messages.len(), 2);
        assert_eq!(req.messages[0].role, Role::User);
        assert_eq!(req.messages[1].role, Role::Assistant);
        assert_eq!(
            req.messages[1].content,
            vec![ContentPart::Text {
                text: "pong".into()
            }]
        );
    }

    #[test]
    fn responses_style_input_text_message_decodes() {
        let req = convert(&json!({
            "model": "gpt-4o",
            "input": [{
                "type": "message",
                "role": "user",
                "content": [
                    {"type": "input_text", "text": "tell me a joke"},
                ],
            }],
        }))
        .unwrap();
        assert_eq!(req.messages.len(), 1);
        assert_eq!(
            req.messages[0].content,
            vec![ContentPart::Text {
                text: "tell me a joke".into()
            }]
        );
    }

    #[test]
    fn responses_style_input_image_decodes_with_object_url() {
        let req = convert(&json!({
            "model": "gpt-4o",
            "input": [{
                "type": "message",
                "role": "user",
                "content": [
                    {"type": "input_image", "image_url": {"url": "https://x/y.png", "detail": "high"}},
                ],
            }],
        }))
        .unwrap();
        assert_eq!(
            req.messages[0].content,
            vec![ContentPart::ImageUrl {
                url: "https://x/y.png".into(),
                detail: Some("high".into()),
            }]
        );
    }

    #[test]
    fn responses_style_input_image_decodes_with_string_url() {
        let req = convert(&json!({
            "model": "gpt-4o",
            "input": [{
                "type": "message",
                "role": "user",
                "content": [
                    {"type": "input_image", "image_url": "https://x/y.png"},
                ],
            }],
        }))
        .unwrap();
        assert_eq!(
            req.messages[0].content,
            vec![ContentPart::ImageUrl {
                url: "https://x/y.png".into(),
                detail: None,
            }]
        );
    }

    #[test]
    fn function_call_attaches_to_previous_assistant_message() {
        let req = convert(&json!({
            "model": "gpt-4o",
            "input": [
                {"role": "user", "content": "what's the weather?"},
                {"role": "assistant", "content": "let me check"},
                {
                    "type": "function_call",
                    "call_id": "call_123",
                    "name": "get_weather",
                    "arguments": "{\"city\":\"SF\"}",
                },
                {
                    "type": "function_call_output",
                    "call_id": "call_123",
                    "output": "72F",
                },
            ],
        }))
        .unwrap();
        assert_eq!(req.messages.len(), 3);
        // assistant message should now hold both text and the tool call.
        let assistant_parts = &req.messages[1].content;
        assert_eq!(assistant_parts.len(), 2);
        assert!(matches!(assistant_parts[0], ContentPart::Text { .. }));
        match &assistant_parts[1] {
            ContentPart::ToolCall(tc) => {
                assert_eq!(tc.id, "call_123");
                assert_eq!(tc.name, "get_weather");
            }
            other => panic!("expected ToolCall, got {other:?}"),
        }
        // function_call_output becomes its own tool message.
        assert_eq!(req.messages[2].role, Role::Tool);
        match &req.messages[2].content[0] {
            ContentPart::ToolResult(tr) => {
                assert_eq!(tr.tool_call_id, "call_123");
                assert_eq!(tr.output, "72F");
            }
            other => panic!("expected ToolResult, got {other:?}"),
        }
    }

    #[test]
    fn function_call_without_prior_assistant_creates_one() {
        let req = convert(&json!({
            "model": "gpt-4o",
            "input": [
                {"role": "user", "content": "fetch"},
                {
                    "type": "function_call",
                    "call_id": "call_1",
                    "name": "fetch",
                    "arguments": "{}",
                },
            ],
        }))
        .unwrap();
        assert_eq!(req.messages.len(), 2);
        assert_eq!(req.messages[1].role, Role::Assistant);
        assert!(matches!(
            req.messages[1].content[0],
            ContentPart::ToolCall(_)
        ));
    }

    #[test]
    fn instructions_prepends_system_message() {
        let req = convert(&json!({
            "model": "gpt-4o",
            "input": "hi",
            "instructions": "you are brief",
        }))
        .unwrap();
        assert_eq!(req.messages.len(), 2);
        assert_eq!(req.messages[0].role, Role::System);
        assert_eq!(
            req.messages[0].content,
            vec![ContentPart::Text {
                text: "you are brief".into()
            }]
        );
        assert_eq!(req.messages[1].role, Role::User);
    }

    #[test]
    fn instructions_wins_over_existing_system_in_input() {
        // Per Responses spec, top-level `instructions` overrides any
        // system message inside `input`. We honor this by prepending,
        // so the override is the first system message the provider
        // sees.
        let req = convert(&json!({
            "model": "gpt-4o",
            "input": [
                {"role": "system", "content": "BE VERBOSE"},
                {"role": "user", "content": "hi"},
            ],
            "instructions": "BE TERSE",
        }))
        .unwrap();
        assert_eq!(req.messages[0].role, Role::System);
        assert_eq!(
            req.messages[0].content,
            vec![ContentPart::Text {
                text: "BE TERSE".into()
            }]
        );
    }

    #[test]
    fn custom_function_tool_is_translated_to_unified() {
        let req = convert(&json!({
            "model": "gpt-4o",
            "input": "x",
            "tools": [{
                "type": "function",
                "name": "lookup",
                "description": "look something up",
                "parameters": {"type": "object"},
            }],
        }))
        .unwrap();
        assert_eq!(req.tools.len(), 1);
        assert_eq!(req.tools[0].name, "lookup");
        assert_eq!(
            req.tools[0].description.as_deref(),
            Some("look something up")
        );
        assert_eq!(req.tools[0].parameters, json!({"type": "object"}));
    }

    #[test]
    fn nested_chat_completions_tool_shape_also_accepted() {
        // Some clients copy/paste the chat-completions nested tool
        // shape into Responses. Accept it for ergonomics.
        let req = convert(&json!({
            "model": "gpt-4o",
            "input": "x",
            "tools": [{
                "type": "function",
                "function": {
                    "name": "ping",
                    "parameters": {"type": "object"},
                },
            }],
        }))
        .unwrap();
        assert_eq!(req.tools.len(), 1);
        assert_eq!(req.tools[0].name, "ping");
    }

    #[test]
    fn builtin_tool_web_search_returns_unsupported_content() {
        let err = convert(&json!({
            "model": "gpt-4o",
            "input": "x",
            "tools": [{"type": "web_search"}],
        }))
        .unwrap_err();
        assert!(matches!(err, ConvertError::UnsupportedContent(_)));
        assert!(err.to_string().contains("web_search"));
    }

    #[test]
    fn missing_model_is_a_missing_field_error() {
        let err = convert(&json!({"input": "hi"})).unwrap_err();
        assert!(matches!(err, ConvertError::MissingField(field) if field == "model"));
    }

    #[test]
    fn missing_input_is_a_missing_field_error() {
        let err = convert(&json!({"model": "gpt-4o"})).unwrap_err();
        assert!(matches!(err, ConvertError::MissingField(field) if field == "input"));
    }

    #[test]
    fn temperature_top_p_max_output_tokens_pass_through() {
        let req = convert(&json!({
            "model": "gpt-4o",
            "input": "x",
            "temperature": 0.4,
            "top_p": 0.85,
            "max_output_tokens": 256,
            "stop": ["\n\n", "END"],
        }))
        .unwrap();
        assert!((req.temperature.unwrap() - 0.4).abs() < 1e-6);
        assert!((req.top_p.unwrap() - 0.85).abs() < 1e-6);
        assert_eq!(req.max_tokens, Some(256));
        assert_eq!(
            req.stop.as_deref(),
            Some(&["\n\n".to_string(), "END".to_string()][..])
        );
    }

    #[test]
    fn stream_flag_passes_through() {
        let req = convert(&json!({
            "model": "gpt-4o",
            "input": "x",
            "stream": true,
        }))
        .unwrap();
        assert!(req.stream);
    }

    #[test]
    fn unknown_top_level_fields_are_silently_ignored() {
        // `previous_response_id`, `store`, `text.format`, `reasoning`,
        // `metadata` are not yet supported but must not error — clients
        // routinely send them and our parser stays forward-compatible.
        let req = convert(&json!({
            "model": "gpt-4o",
            "input": "hi",
            "previous_response_id": "resp_abc",
            "store": true,
            "text": {"format": {"type": "json_schema"}},
            "reasoning": {"effort": "low"},
            "metadata": {"trace_id": "xyz"},
        }))
        .unwrap();
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.messages[0].role, Role::User);
    }
}
