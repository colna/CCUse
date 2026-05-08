//! Integration test: format compatibility matrix (T1.0.3.13-14).
//!
//! Verifies that a request in any of the 3 client formats (`OpenAI`,
//! Anthropic, Gemini) can be converted to unified, then re-encoded
//! into any of the 3 vendor formats — and the same for responses
//! and streaming chunks.  That's a 3 × 3 × 2 (stream + non-stream)
//! = 18-cell matrix.

use ccuse_desktop_lib::converter::{
    AnthropicConverter, ConvertError, FormatConverter, GeminiConverter, OpenAIConverter,
};
use serde_json::json;

// ─── Helpers ─────────────────────────────────────────────────

fn openai() -> OpenAIConverter {
    OpenAIConverter::new()
}
fn anthropic() -> AnthropicConverter {
    AnthropicConverter::new()
}
fn gemini() -> GeminiConverter {
    GeminiConverter::new()
}

/// Convert a client request body through the unified format into a
/// vendor-specific request body, returning the result.
fn convert_request(
    from: &dyn FormatConverter,
    to: &dyn FormatConverter,
    body: &serde_json::Value,
) -> Result<serde_json::Value, ConvertError> {
    let unified = from.request_to_unified(body)?;
    to.unified_to_request(&unified)
}

/// Convert a vendor response body through the unified format into a
/// client-specific response body, returning the result.
fn convert_response(
    from: &dyn FormatConverter,
    to: &dyn FormatConverter,
    body: &serde_json::Value,
) -> Result<serde_json::Value, ConvertError> {
    let unified = from.response_to_unified(body)?;
    to.unified_to_response(&unified)
}

/// Convert a stream chunk through parse + encode.
fn convert_stream(
    from: &dyn FormatConverter,
    to: &dyn FormatConverter,
    data: &str,
) -> Result<Option<String>, ConvertError> {
    let chunk = from.parse_stream_chunk(data)?;
    match chunk {
        Some(c) => Ok(Some(to.encode_stream_chunk(&c)?)),
        None => Ok(None),
    }
}

// ─── Standard test payloads ──────────────────────────────────

fn openai_request() -> serde_json::Value {
    json!({
        "model": "gpt-5.5-instant",
        "messages": [
            {"role": "system", "content": "You are helpful."},
            {"role": "user", "content": "Hello!"}
        ],
        "temperature": 0.7,
        "max_tokens": 100,
        "stream": false
    })
}

fn anthropic_request() -> serde_json::Value {
    json!({
        "model": "claude-sonnet-4-6",
        "system": "You are helpful.",
        "messages": [
            {"role": "user", "content": "Hello!"}
        ],
        "max_tokens": 100,
        "temperature": 0.7
    })
}

fn gemini_request() -> serde_json::Value {
    json!({
        "contents": [
            {"role": "user", "parts": [{"text": "Hello!"}]}
        ],
        "system_instruction": {"parts": [{"text": "You are helpful."}]},
        "generationConfig": {"temperature": 0.7, "maxOutputTokens": 100}
    })
}

fn openai_response() -> serde_json::Value {
    json!({
        "id": "chatcmpl-abc",
        "object": "chat.completion",
        "model": "gpt-5.5-instant",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "Hi there!"},
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
    })
}

fn anthropic_response() -> serde_json::Value {
    json!({
        "id": "msg_abc",
        "type": "message",
        "role": "assistant",
        "model": "claude-sonnet-4-6",
        "content": [{"type": "text", "text": "Hi there!"}],
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 10, "output_tokens": 5}
    })
}

fn gemini_response() -> serde_json::Value {
    json!({
        "candidates": [{
            "content": {"role": "model", "parts": [{"text": "Hi there!"}]},
            "finishReason": "STOP"
        }],
        "usageMetadata": {"promptTokenCount": 10, "candidatesTokenCount": 5, "totalTokenCount": 15}
    })
}

fn openai_stream_chunk() -> String {
    json!({
        "id": "chatcmpl-1",
        "model": "gpt-5.5-instant",
        "choices": [{
            "index": 0,
            "delta": {"role": "assistant", "content": "Hi"},
            "finish_reason": null
        }]
    })
    .to_string()
}

fn anthropic_stream_chunk() -> String {
    json!({
        "type": "content_block_delta",
        "index": 0,
        "delta": {"type": "text_delta", "text": "Hi"}
    })
    .to_string()
}

fn gemini_stream_chunk() -> String {
    json!({
        "candidates": [{
            "content": {"role": "model", "parts": [{"text": "Hi"}]}
        }]
    })
    .to_string()
}

// ═══════════════════════════════════════════════════════════════
// T1.0.3.13 — Edge case unit tests for each converter
// ═══════════════════════════════════════════════════════════════

mod openai_edge_cases {
    use super::*;

    #[test]
    fn empty_messages_array() {
        let input = json!({"model": "gpt-5.5-instant", "messages": [], "stream": false});
        let u = openai().request_to_unified(&input).unwrap();
        assert!(u.messages.is_empty());
    }

    #[test]
    fn null_content_field() {
        let input = json!({
            "model": "gpt-5.5-instant",
            "messages": [{"role": "assistant", "content": null}],
            "stream": false
        });
        let u = openai().request_to_unified(&input).unwrap();
        assert!(u.messages[0].content.is_empty());
    }

    #[test]
    fn response_without_usage() {
        let input = json!({
            "id": "x",
            "model": "gpt-5.5-instant",
            "choices": [{"index": 0, "message": {"role": "assistant", "content": "ok"}, "finish_reason": "stop"}]
        });
        let u = openai().response_to_unified(&input).unwrap();
        assert!(u.usage.is_none());
    }

    #[test]
    fn stream_chunk_with_usage() {
        let data = json!({
            "id": "c1", "model": "gpt-5.5-instant",
            "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 5, "completion_tokens": 3, "total_tokens": 8}
        });
        let chunk = openai()
            .parse_stream_chunk(&data.to_string())
            .unwrap()
            .unwrap();
        assert_eq!(chunk.usage.as_ref().unwrap().total_tokens, 8);
    }

    #[test]
    fn multiple_tool_calls() {
        let input = json!({
            "model": "gpt-5.5-instant",
            "messages": [{
                "role": "assistant",
                "content": null,
                "tool_calls": [
                    {"id": "c1", "type": "function", "function": {"name": "a", "arguments": "{}"}},
                    {"id": "c2", "type": "function", "function": {"name": "b", "arguments": "{}"}}
                ]
            }],
            "stream": false
        });
        let u = openai().request_to_unified(&input).unwrap();
        assert_eq!(u.messages[0].tool_calls().len(), 2);
    }

    #[test]
    fn stop_sequences() {
        let input = json!({
            "model": "gpt-5.5-instant",
            "messages": [{"role": "user", "content": "hi"}],
            "stop": ["END", "STOP"],
            "stream": false
        });
        let u = openai().request_to_unified(&input).unwrap();
        assert_eq!(u.stop.as_ref().unwrap().len(), 2);
    }
}

mod anthropic_edge_cases {
    use super::*;

    #[test]
    fn system_as_array() {
        let input = json!({
            "model": "claude-sonnet-4-6",
            "max_tokens": 100,
            "system": [{"type": "text", "text": "part1"}, {"type": "text", "text": "part2"}],
            "messages": [{"role": "user", "content": "hi"}]
        });
        let u = anthropic().request_to_unified(&input).unwrap();
        assert_eq!(u.messages[0].text_content(), "part1part2");
    }

    #[test]
    fn tool_result_with_array_content() {
        let input = json!({
            "model": "claude-sonnet-4-6",
            "max_tokens": 100,
            "messages": [{
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": "tu_1",
                    "content": [{"type": "text", "text": "result data"}]
                }]
            }]
        });
        let u = anthropic().request_to_unified(&input).unwrap();
        assert!(u.messages[0].content.iter().any(|p| {
            matches!(p, ccuse_desktop_lib::converter::ContentPart::ToolResult(tr) if tr.output == "result data")
        }));
    }

    #[test]
    fn response_tool_use_content() {
        let input = json!({
            "id": "msg_1",
            "model": "claude-sonnet-4-6",
            "content": [
                {"type": "text", "text": "Let me search."},
                {"type": "tool_use", "id": "tu_1", "name": "search", "input": {"q": "rust"}}
            ],
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 5, "output_tokens": 10}
        });
        let u = anthropic().response_to_unified(&input).unwrap();
        let tc = u.choices[0].message.tool_calls();
        assert_eq!(tc.len(), 1);
        assert_eq!(
            u.choices[0].finish_reason,
            Some(ccuse_desktop_lib::converter::FinishReason::ToolCalls)
        );
    }

    #[test]
    fn max_tokens_default() {
        let u = ccuse_desktop_lib::converter::UnifiedRequest {
            model: "claude".into(),
            messages: vec![],
            temperature: None,
            max_tokens: None,
            top_p: None,
            stop: None,
            stream: false,
            tools: vec![],
        };
        let back = anthropic().unified_to_request(&u).unwrap();
        assert_eq!(back["max_tokens"], 4096); // Default
    }

    #[test]
    fn stream_message_delta_with_usage() {
        let data = json!({
            "type": "message_delta",
            "delta": {"stop_reason": "end_turn"},
            "usage": {"output_tokens": 42}
        });
        let chunk = anthropic()
            .parse_stream_chunk(&data.to_string())
            .unwrap()
            .unwrap();
        assert_eq!(chunk.usage.as_ref().unwrap().completion_tokens, 42);
    }
}

mod gemini_edge_cases {
    use super::*;

    #[test]
    fn no_system_instruction() {
        let input = json!({
            "contents": [{"role": "user", "parts": [{"text": "hi"}]}]
        });
        let u = gemini().request_to_unified(&input).unwrap();
        assert_eq!(u.messages.len(), 1);
    }

    #[test]
    fn multiple_candidates() {
        let input = json!({
            "candidates": [
                {"content": {"role": "model", "parts": [{"text": "A"}]}, "finishReason": "STOP"},
                {"content": {"role": "model", "parts": [{"text": "B"}]}, "finishReason": "STOP"}
            ]
        });
        let u = gemini().response_to_unified(&input).unwrap();
        assert_eq!(u.choices.len(), 2);
        assert_eq!(u.choices[1].message.text_content(), "B");
    }

    #[test]
    fn safety_finish_reason() {
        let input = json!({
            "candidates": [{
                "content": {"role": "model", "parts": [{"text": ""}]},
                "finishReason": "SAFETY"
            }]
        });
        let u = gemini().response_to_unified(&input).unwrap();
        assert_eq!(
            u.choices[0].finish_reason,
            Some(ccuse_desktop_lib::converter::FinishReason::ContentFilter)
        );
    }

    #[test]
    fn stream_usage_only_chunk() {
        let data = json!({
            "usageMetadata": {"promptTokenCount": 10, "candidatesTokenCount": 5, "totalTokenCount": 15}
        });
        let chunk = gemini()
            .parse_stream_chunk(&data.to_string())
            .unwrap()
            .unwrap();
        assert!(chunk.choices.is_empty());
        assert_eq!(chunk.usage.as_ref().unwrap().total_tokens, 15);
    }

    #[test]
    fn model_defaults_to_gemini() {
        let input = json!({
            "contents": [{"role": "user", "parts": [{"text": "hi"}]}]
        });
        let u = gemini().request_to_unified(&input).unwrap();
        assert_eq!(u.model, "gemini-1.5-pro");
    }
}

// ═══════════════════════════════════════════════════════════════
// T1.0.3.14 — Compatibility integration matrix (3×3×2)
// ═══════════════════════════════════════════════════════════════

/// Test that text content survives cross-format conversion.
fn assert_text_preserved(result: &serde_json::Value, format: &str) {
    match format {
        "openai" => {
            // Find "Hello!" somewhere in messages.
            let msgs = result["messages"].as_array().unwrap();
            let has_hello = msgs.iter().any(|m| {
                m["content"].as_str().is_some_and(|s| s.contains("Hello"))
                    || m["content"].as_array().is_some_and(|arr| {
                        arr.iter()
                            .any(|p| p["text"].as_str().is_some_and(|s| s.contains("Hello")))
                    })
            });
            assert!(has_hello, "OpenAI request should contain 'Hello'");
        }
        "anthropic" => {
            let msgs = result["messages"].as_array().unwrap();
            let has_hello = msgs.iter().any(|m| {
                m["content"].as_str().is_some_and(|s| s.contains("Hello"))
                    || m["content"].as_array().is_some_and(|arr| {
                        arr.iter()
                            .any(|p| p["text"].as_str().is_some_and(|s| s.contains("Hello")))
                    })
            });
            assert!(has_hello, "Anthropic request should contain 'Hello'");
        }
        "gemini" => {
            let contents = result["contents"].as_array().unwrap();
            let has_hello = contents.iter().any(|c| {
                c["parts"].as_array().is_some_and(|arr| {
                    arr.iter()
                        .any(|p| p["text"].as_str().is_some_and(|s| s.contains("Hello")))
                })
            });
            assert!(has_hello, "Gemini request should contain 'Hello'");
        }
        _ => panic!("Unknown format: {format}"),
    }
}

// ── Non-streaming request matrix (3×3 = 9 tests) ────────────

#[test]
fn openai_to_openai_request() {
    let result = convert_request(&openai(), &openai(), &openai_request()).unwrap();
    assert_text_preserved(&result, "openai");
}

#[test]
fn openai_to_anthropic_request() {
    let result = convert_request(&openai(), &anthropic(), &openai_request()).unwrap();
    assert_text_preserved(&result, "anthropic");
    assert!(result["system"].as_str().is_some()); // System extracted.
}

#[test]
fn openai_to_gemini_request() {
    let result = convert_request(&openai(), &gemini(), &openai_request()).unwrap();
    assert_text_preserved(&result, "gemini");
    assert!(result["system_instruction"].is_object());
}

#[test]
fn anthropic_to_openai_request() {
    let result = convert_request(&anthropic(), &openai(), &anthropic_request()).unwrap();
    assert_text_preserved(&result, "openai");
}

#[test]
fn anthropic_to_anthropic_request() {
    let result = convert_request(&anthropic(), &anthropic(), &anthropic_request()).unwrap();
    assert_text_preserved(&result, "anthropic");
}

#[test]
fn anthropic_to_gemini_request() {
    let result = convert_request(&anthropic(), &gemini(), &anthropic_request()).unwrap();
    assert_text_preserved(&result, "gemini");
}

#[test]
fn gemini_to_openai_request() {
    let result = convert_request(&gemini(), &openai(), &gemini_request()).unwrap();
    assert_text_preserved(&result, "openai");
}

#[test]
fn gemini_to_anthropic_request() {
    let result = convert_request(&gemini(), &anthropic(), &gemini_request()).unwrap();
    assert_text_preserved(&result, "anthropic");
}

#[test]
fn gemini_to_gemini_request() {
    let result = convert_request(&gemini(), &gemini(), &gemini_request()).unwrap();
    assert_text_preserved(&result, "gemini");
}

// ── Non-streaming response matrix (3×3 = 9 tests) ───────────

fn assert_response_text(result: &serde_json::Value, format: &str) {
    match format {
        "openai" => {
            let text = result["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("");
            assert!(text.contains("Hi"), "OpenAI response: {text}");
        }
        "anthropic" => {
            let arr = result["content"].as_array().unwrap();
            let text = arr
                .iter()
                .filter_map(|b| b["text"].as_str())
                .collect::<String>();
            assert!(text.contains("Hi"), "Anthropic response: {text}");
        }
        "gemini" => {
            let parts = result["candidates"][0]["content"]["parts"]
                .as_array()
                .unwrap();
            let text = parts
                .iter()
                .filter_map(|p| p["text"].as_str())
                .collect::<String>();
            assert!(text.contains("Hi"), "Gemini response: {text}");
        }
        _ => panic!("Unknown format: {format}"),
    }
}

#[test]
fn openai_to_openai_response() {
    let result = convert_response(&openai(), &openai(), &openai_response()).unwrap();
    assert_response_text(&result, "openai");
}

#[test]
fn openai_to_anthropic_response() {
    let result = convert_response(&openai(), &anthropic(), &openai_response()).unwrap();
    assert_response_text(&result, "anthropic");
}

#[test]
fn openai_to_gemini_response() {
    let result = convert_response(&openai(), &gemini(), &openai_response()).unwrap();
    assert_response_text(&result, "gemini");
}

#[test]
fn anthropic_to_openai_response() {
    let result = convert_response(&anthropic(), &openai(), &anthropic_response()).unwrap();
    assert_response_text(&result, "openai");
}

#[test]
fn anthropic_to_anthropic_response() {
    let result = convert_response(&anthropic(), &anthropic(), &anthropic_response()).unwrap();
    assert_response_text(&result, "anthropic");
}

#[test]
fn anthropic_to_gemini_response() {
    let result = convert_response(&anthropic(), &gemini(), &anthropic_response()).unwrap();
    assert_response_text(&result, "gemini");
}

#[test]
fn gemini_to_openai_response() {
    let result = convert_response(&gemini(), &openai(), &gemini_response()).unwrap();
    assert_response_text(&result, "openai");
}

#[test]
fn gemini_to_anthropic_response() {
    let result = convert_response(&gemini(), &anthropic(), &gemini_response()).unwrap();
    assert_response_text(&result, "anthropic");
}

#[test]
fn gemini_to_gemini_response() {
    let result = convert_response(&gemini(), &gemini(), &gemini_response()).unwrap();
    assert_response_text(&result, "gemini");
}

// ── Streaming matrix (3×3 = 9 tests) ────────────────────────

#[test]
fn openai_stream_to_openai() {
    let result = convert_stream(&openai(), &openai(), &openai_stream_chunk()).unwrap();
    assert!(result.unwrap().contains("Hi"));
}

#[test]
fn openai_stream_to_anthropic() {
    let result = convert_stream(&openai(), &anthropic(), &openai_stream_chunk()).unwrap();
    assert!(result.unwrap().contains("Hi"));
}

#[test]
fn openai_stream_to_gemini() {
    let result = convert_stream(&openai(), &gemini(), &openai_stream_chunk()).unwrap();
    assert!(result.unwrap().contains("Hi"));
}

#[test]
fn anthropic_stream_to_openai() {
    let result = convert_stream(&anthropic(), &openai(), &anthropic_stream_chunk()).unwrap();
    assert!(result.unwrap().contains("Hi"));
}

#[test]
fn anthropic_stream_to_anthropic() {
    let result = convert_stream(&anthropic(), &anthropic(), &anthropic_stream_chunk()).unwrap();
    assert!(result.unwrap().contains("Hi"));
}

#[test]
fn anthropic_stream_to_gemini() {
    let result = convert_stream(&anthropic(), &gemini(), &anthropic_stream_chunk()).unwrap();
    assert!(result.unwrap().contains("Hi"));
}

#[test]
fn gemini_stream_to_openai() {
    let result = convert_stream(&gemini(), &openai(), &gemini_stream_chunk()).unwrap();
    assert!(result.unwrap().contains("Hi"));
}

#[test]
fn gemini_stream_to_anthropic() {
    let result = convert_stream(&gemini(), &anthropic(), &gemini_stream_chunk()).unwrap();
    assert!(result.unwrap().contains("Hi"));
}

#[test]
fn gemini_stream_to_gemini() {
    let result = convert_stream(&gemini(), &gemini(), &gemini_stream_chunk()).unwrap();
    assert!(result.unwrap().contains("Hi"));
}

// ── Tool calling cross-format (3 pairs) ─────────────────────

fn openai_tool_request() -> serde_json::Value {
    json!({
        "model": "gpt-5.5-instant",
        "messages": [
            {"role": "user", "content": "weather?"},
            {
                "role": "assistant", "content": null,
                "tool_calls": [{"id": "c1", "type": "function", "function": {"name": "get_weather", "arguments": "{\"loc\":\"Tokyo\"}"}}]
            },
            {"role": "tool", "tool_call_id": "c1", "content": "sunny 25C"}
        ],
        "tools": [{"type": "function", "function": {"name": "get_weather", "description": "Weather", "parameters": {"type": "object"}}}],
        "stream": false
    })
}

#[test]
fn tool_call_openai_to_anthropic() {
    let result = convert_request(&openai(), &anthropic(), &openai_tool_request()).unwrap();
    // Should have tool_use block in assistant message.
    let msgs = result["messages"].as_array().unwrap();
    let has_tool_use = msgs.iter().any(|m| {
        m["content"]
            .as_array()
            .is_some_and(|arr| arr.iter().any(|b| b["type"] == "tool_use"))
    });
    assert!(
        has_tool_use,
        "Anthropic request should contain tool_use block"
    );
    // Tools should use input_schema.
    assert!(result["tools"][0]["input_schema"].is_object());
}

#[test]
fn tool_call_openai_to_gemini() {
    let result = convert_request(&openai(), &gemini(), &openai_tool_request()).unwrap();
    let contents = result["contents"].as_array().unwrap();
    let has_fn_call = contents.iter().any(|c| {
        c["parts"]
            .as_array()
            .is_some_and(|arr| arr.iter().any(|p| p["functionCall"].is_object()))
    });
    assert!(has_fn_call, "Gemini request should contain functionCall");
    assert!(result["tools"][0]["functionDeclarations"].is_array());
}

#[test]
fn tool_call_anthropic_to_openai() {
    let input = json!({
        "model": "claude-sonnet-4-6",
        "max_tokens": 100,
        "messages": [
            {"role": "user", "content": "weather?"},
            {"role": "assistant", "content": [
                {"type": "tool_use", "id": "tu_1", "name": "get_weather", "input": {"loc": "Tokyo"}}
            ]},
            {"role": "user", "content": [
                {"type": "tool_result", "tool_use_id": "tu_1", "content": "sunny"}
            ]}
        ]
    });
    let result = convert_request(&anthropic(), &openai(), &input).unwrap();
    let msgs = result["messages"].as_array().unwrap();
    // Assistant should have tool_calls.
    assert!(msgs[1]["tool_calls"].is_array());
    // Tool message should have tool_call_id.
    assert!(msgs[2]["tool_call_id"].is_string());
}
