//! `OpenAI` Responses API (`POST /v1/responses`) codec
//! (T1.0.6.36, T1.0.6.37, T1.0.6.38).
//!
//! Responses is the newer agentic surface that ships with the
//! `responses.create()` SDK call, Codex CLI, and several IDE
//! integrations. Its request shape is a superset of chat-completions
//! with a richer `input` schema (string / array of typed items),
//! `instructions` as a top-level system override, a flatter `tools`
//! array, and stateful conversation fields we deliberately do not yet
//! implement.
//!
//! This module covers all three codec halves the proxy needs:
//! [`ResponsesConverter::request_to_unified`] (T1.0.6.36) parses the
//! request, [`ResponsesConverter::unified_to_response`] (T1.0.6.37)
//! serialises non-streaming responses, and [`ResponsesStreamEncoder`]
//! (T1.0.6.38) drives the typed SSE event sequence for streaming.

use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use uuid::Uuid;

use super::types::{
    ContentPart, FinishReason, Role, StreamToolCall, ToolCall, ToolDefinition, ToolResult,
    UnifiedMessage, UnifiedRequest, UnifiedResponse, UnifiedStreamChunk, UnifiedUsage,
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

    /// Serialise a [`UnifiedResponse`] into the Responses API JSON shape.
    ///
    /// `request_model` is what the client sent; we echo it back instead
    /// of whatever the upstream provider may have rewritten so clients
    /// can correlate request and response without surprises.
    ///
    /// The output array follows `OpenAI`'s contract:
    /// - One `message` item with the assistant text (only emitted if
    ///   there is text to send back).
    /// - Zero or more `function_call` items, in the order the unified
    ///   message produced them.
    ///
    /// `finish_reason` maps onto the response-level `status` field per
    /// the spec: `Stop` / `ToolCalls` → `completed`, `Length` →
    /// `incomplete` with `incomplete_details.reason = "max_output_tokens"`,
    /// `ContentFilter` → `incomplete` with reason `"content_filter"`.
    pub fn unified_to_response(
        &self,
        resp: &UnifiedResponse,
        request_model: &str,
    ) -> Result<Value, ConvertError> {
        let response_id = format!("resp_{}", Uuid::new_v4().simple());
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Take the first choice — chat-completions providers virtually
        // always return one. If a provider ever streams `n > 1` we
        // surface only the primary completion; tracking branches in
        // Responses output isn't part of the MVP.
        let primary = resp.choices.first();
        let finish_reason = primary.and_then(|c| c.finish_reason);
        let (status, incomplete_details) = status_for_finish_reason(finish_reason);

        let mut output_items: Vec<Value> = Vec::new();
        if let Some(choice) = primary {
            collect_output_items(&choice.message, &mut output_items);
        }

        let model = if request_model.is_empty() {
            resp.model.clone()
        } else {
            request_model.to_owned()
        };

        let usage = resp.usage.as_ref().map_or_else(
            || {
                json!({
                    "input_tokens": 0,
                    "output_tokens": 0,
                    "total_tokens": 0,
                })
            },
            |u| {
                json!({
                    "input_tokens": u.prompt_tokens,
                    "output_tokens": u.completion_tokens,
                    "total_tokens": u.total_tokens,
                })
            },
        );

        Ok(json!({
            "id": response_id,
            "object": "response",
            "created_at": created_at,
            "status": status,
            "model": model,
            "output": output_items,
            "usage": usage,
            "incomplete_details": incomplete_details,
            "error": Value::Null,
            "metadata": json!({}),
        }))
    }
}

/// Pour a unified assistant message into the Responses `output` array
/// shape. Text becomes a single `message` item with `output_text`
/// content; each tool call becomes its own `function_call` item.
fn collect_output_items(message: &UnifiedMessage, out: &mut Vec<Value>) {
    let mut text_parts: Vec<&str> = Vec::new();
    let mut tool_calls: Vec<&ToolCall> = Vec::new();
    for part in &message.content {
        match part {
            ContentPart::Text { text } if !text.is_empty() => text_parts.push(text),
            ContentPart::ToolCall(call) => tool_calls.push(call),
            // ImageUrl / ToolResult never appear on assistant output;
            // skip them silently — they're handled on the input side.
            _ => {}
        }
    }

    if !text_parts.is_empty() {
        let text = text_parts.join("");
        out.push(json!({
            "id": format!("msg_{}", Uuid::new_v4().simple()),
            "type": "message",
            "role": "assistant",
            "status": "completed",
            "content": [
                {
                    "type": "output_text",
                    "text": text,
                    "annotations": [],
                }
            ],
        }));
    }

    for call in tool_calls {
        out.push(json!({
            "id": format!("fc_{}", Uuid::new_v4().simple()),
            "type": "function_call",
            "status": "completed",
            "call_id": call.id,
            "name": call.name,
            "arguments": call.arguments,
        }));
    }
}

/// Map a [`FinishReason`] onto the Responses `status` +
/// `incomplete_details` pair.
fn status_for_finish_reason(reason: Option<FinishReason>) -> (&'static str, Value) {
    match reason {
        Some(FinishReason::Length) => ("incomplete", json!({"reason": "max_output_tokens"})),
        Some(FinishReason::ContentFilter) => ("incomplete", json!({"reason": "content_filter"})),
        // Stop / ToolCalls / unknown all read as a clean completion;
        // a missing finish_reason during streaming is also benign here
        // because non-streaming responses always have one.
        _ => ("completed", Value::Null),
    }
}

// ─── streaming (T1.0.6.38) ──────────────────────────────────────────────

/// Stateful encoder that converts a stream of [`UnifiedStreamChunk`]
/// into the Responses API's typed SSE event sequence.
///
/// The Responses spec demands a strict per-output-item lifecycle:
///
/// ```text
/// response.created → response.in_progress
///   ↓
///   ── for each text message ─────────────────────────────────
///   response.output_item.added (message, in_progress)
///   response.content_part.added (output_text, "")
///   response.output_text.delta * N   (one per chunk)
///   response.output_text.done
///   response.content_part.done
///   response.output_item.done
///
///   ── for each tool call ────────────────────────────────────
///   response.output_item.added (function_call, in_progress)
///   response.function_call_arguments.delta * N
///   response.function_call_arguments.done
///   response.output_item.done
///   ↓
/// response.completed   (or `error` on upstream failure)
/// ```
///
/// We hold the in-flight item in [`StreamState`] and emit the
/// open/close frames lazily as the underlying chat-completions chunks
/// reveal a switch from text to a tool call (or back). This way the
/// encoder is independent of any specific upstream provider and
/// works for both `OpenAI` and Anthropic source streams (which the
/// handler unifies first via `OpenAIConverter::parse_stream_chunk`).
#[derive(Debug)]
pub struct ResponsesStreamEncoder {
    response_id: String,
    request_model: String,
    created_at: u64,
    next_output_index: u32,
    state: StreamState,
    completed_items: Vec<Value>,
    started: bool,
    finish_reason: Option<FinishReason>,
}

#[derive(Debug)]
enum StreamState {
    Idle,
    Text {
        item_id: String,
        output_index: u32,
        accumulated: String,
    },
    ToolCall {
        item_id: String,
        output_index: u32,
        call_id: String,
        name: String,
        accumulated_args: String,
    },
}

impl ResponsesStreamEncoder {
    #[must_use]
    pub fn new(request_model: String) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        Self {
            response_id: format!("resp_{}", Uuid::new_v4().simple()),
            request_model,
            created_at: now,
            next_output_index: 0,
            state: StreamState::Idle,
            completed_items: Vec::new(),
            started: false,
            finish_reason: None,
        }
    }

    /// Emit `response.created` + `response.in_progress`. Idempotent —
    /// safe to call twice; second call returns an empty vector. Call
    /// before the first upstream chunk arrives so clients see the
    /// response envelope immediately, even if the model is slow.
    pub fn start_frames(&mut self) -> Vec<String> {
        if self.started {
            return Vec::new();
        }
        self.started = true;
        let snapshot = self.snapshot("in_progress", &[]);
        vec![
            format_sse_frame(
                "response.created",
                &json!({"type": "response.created", "response": snapshot}),
            ),
            format_sse_frame(
                "response.in_progress",
                &json!({"type": "response.in_progress", "response": snapshot}),
            ),
        ]
    }

    /// Translate a single chat-completions–shaped chunk into zero or
    /// more Responses SSE frames. Drives the per-item state machine.
    pub fn process_chunk(&mut self, chunk: &UnifiedStreamChunk) -> Vec<String> {
        let mut frames = Vec::new();
        for choice in &chunk.choices {
            if let Some(delta) = &choice.delta {
                if let Some(content) = delta.content.as_deref() {
                    if !content.is_empty() {
                        frames.extend(self.ingest_text(content));
                    }
                }
                for tool_call in &delta.tool_calls {
                    frames.extend(self.ingest_tool_call(tool_call));
                }
            }
            if let Some(reason) = choice.finish_reason {
                self.finish_reason = Some(reason);
            }
        }
        frames
    }

    /// Close the in-flight item (if any) and emit `response.completed`.
    /// Should be called exactly once when the upstream stream ends.
    pub fn finalize(&mut self, final_usage: Option<&UnifiedUsage>) -> Vec<String> {
        let mut frames = self.close_current_state();
        let usage_json = final_usage.map_or_else(
            || json!({"input_tokens": 0, "output_tokens": 0, "total_tokens": 0}),
            |u| {
                json!({
                    "input_tokens": u.prompt_tokens,
                    "output_tokens": u.completion_tokens,
                    "total_tokens": u.total_tokens,
                })
            },
        );
        let (status, incomplete_details) = status_for_finish_reason(self.finish_reason);
        let snapshot = json!({
            "id": self.response_id,
            "object": "response",
            "created_at": self.created_at,
            "status": status,
            "model": self.request_model,
            "output": self.completed_items.clone(),
            "usage": usage_json,
            "incomplete_details": incomplete_details,
            "error": Value::Null,
            "metadata": json!({}),
        });
        frames.push(format_sse_frame(
            "response.completed",
            &json!({
                "type": "response.completed",
                "response": snapshot,
            }),
        ));
        frames
    }

    /// Emit an `error` event for an upstream failure mid-stream. Caller
    /// owns deciding whether to also send `response.completed` after.
    /// Any open item is closed first so the client doesn't see a
    /// dangling `in_progress`.
    pub fn error_frame(&mut self, message: &str) -> Vec<String> {
        let mut frames = self.close_current_state();
        frames.push(format_sse_frame(
            "error",
            &json!({
                "type": "error",
                "code": "upstream_error",
                "message": message,
            }),
        ));
        frames
    }

    fn ingest_text(&mut self, text: &str) -> Vec<String> {
        let mut frames = Vec::new();
        if matches!(self.state, StreamState::ToolCall { .. }) {
            frames.extend(self.close_current_state());
        }
        if matches!(self.state, StreamState::Idle) {
            let item_id = format!("msg_{}", Uuid::new_v4().simple());
            let output_index = self.next_output_index;
            self.next_output_index += 1;
            frames.push(format_sse_frame(
                "response.output_item.added",
                &json!({
                    "type": "response.output_item.added",
                    "output_index": output_index,
                    "item": {
                        "id": item_id,
                        "type": "message",
                        "role": "assistant",
                        "status": "in_progress",
                        "content": [],
                    },
                }),
            ));
            frames.push(format_sse_frame(
                "response.content_part.added",
                &json!({
                    "type": "response.content_part.added",
                    "item_id": item_id,
                    "output_index": output_index,
                    "content_index": 0,
                    "part": {"type": "output_text", "text": "", "annotations": []},
                }),
            ));
            self.state = StreamState::Text {
                item_id,
                output_index,
                accumulated: String::new(),
            };
        }
        if let StreamState::Text {
            item_id,
            output_index,
            accumulated,
        } = &mut self.state
        {
            accumulated.push_str(text);
            frames.push(format_sse_frame(
                "response.output_text.delta",
                &json!({
                    "type": "response.output_text.delta",
                    "item_id": item_id,
                    "output_index": *output_index,
                    "content_index": 0,
                    "delta": text,
                }),
            ));
        }
        frames
    }

    fn ingest_tool_call(&mut self, tool_call: &StreamToolCall) -> Vec<String> {
        let mut frames = Vec::new();
        // Decide whether this delta belongs to a brand-new function
        // call item. `tool_call.id` is the upstream call id; when it
        // shows up and disagrees with what we're tracking, we know the
        // model has started a new tool invocation.
        let starts_new_call = match &self.state {
            StreamState::Idle | StreamState::Text { .. } => true,
            StreamState::ToolCall { call_id, .. } => tool_call
                .id
                .as_deref()
                .is_some_and(|id| !id.is_empty() && id != call_id),
        };
        if starts_new_call {
            frames.extend(self.close_current_state());
            let item_id = format!("fc_{}", Uuid::new_v4().simple());
            let output_index = self.next_output_index;
            self.next_output_index += 1;
            let call_id = tool_call.id.clone().unwrap_or_default();
            let name = tool_call.name.clone().unwrap_or_default();
            frames.push(format_sse_frame(
                "response.output_item.added",
                &json!({
                    "type": "response.output_item.added",
                    "output_index": output_index,
                    "item": {
                        "id": item_id,
                        "type": "function_call",
                        "status": "in_progress",
                        "call_id": call_id,
                        "name": name,
                        "arguments": "",
                    },
                }),
            ));
            self.state = StreamState::ToolCall {
                item_id,
                output_index,
                call_id,
                name,
                accumulated_args: String::new(),
            };
        }
        if let StreamState::ToolCall {
            item_id,
            output_index,
            accumulated_args,
            ..
        } = &mut self.state
        {
            if let Some(args) = tool_call.arguments.as_deref() {
                if !args.is_empty() {
                    accumulated_args.push_str(args);
                    frames.push(format_sse_frame(
                        "response.function_call_arguments.delta",
                        &json!({
                            "type": "response.function_call_arguments.delta",
                            "item_id": item_id,
                            "output_index": *output_index,
                            "delta": args,
                        }),
                    ));
                }
            }
        }
        frames
    }

    fn close_current_state(&mut self) -> Vec<String> {
        let mut frames = Vec::new();
        match std::mem::replace(&mut self.state, StreamState::Idle) {
            StreamState::Idle => {}
            StreamState::Text {
                item_id,
                output_index,
                accumulated,
            } => {
                frames.push(format_sse_frame(
                    "response.output_text.done",
                    &json!({
                        "type": "response.output_text.done",
                        "item_id": item_id,
                        "output_index": output_index,
                        "content_index": 0,
                        "text": accumulated,
                    }),
                ));
                frames.push(format_sse_frame(
                    "response.content_part.done",
                    &json!({
                        "type": "response.content_part.done",
                        "item_id": item_id,
                        "output_index": output_index,
                        "content_index": 0,
                        "part": {
                            "type": "output_text",
                            "text": accumulated,
                            "annotations": [],
                        },
                    }),
                ));
                let final_item = json!({
                    "id": item_id,
                    "type": "message",
                    "role": "assistant",
                    "status": "completed",
                    "content": [
                        {"type": "output_text", "text": accumulated, "annotations": []}
                    ],
                });
                frames.push(format_sse_frame(
                    "response.output_item.done",
                    &json!({
                        "type": "response.output_item.done",
                        "output_index": output_index,
                        "item": final_item,
                    }),
                ));
                self.completed_items.push(final_item);
            }
            StreamState::ToolCall {
                item_id,
                output_index,
                call_id,
                name,
                accumulated_args,
            } => {
                frames.push(format_sse_frame(
                    "response.function_call_arguments.done",
                    &json!({
                        "type": "response.function_call_arguments.done",
                        "item_id": item_id,
                        "output_index": output_index,
                        "arguments": accumulated_args,
                    }),
                ));
                let final_item = json!({
                    "id": item_id,
                    "type": "function_call",
                    "status": "completed",
                    "call_id": call_id,
                    "name": name,
                    "arguments": accumulated_args,
                });
                frames.push(format_sse_frame(
                    "response.output_item.done",
                    &json!({
                        "type": "response.output_item.done",
                        "output_index": output_index,
                        "item": final_item,
                    }),
                ));
                self.completed_items.push(final_item);
            }
        }
        frames
    }

    fn snapshot(&self, status: &str, items: &[Value]) -> Value {
        json!({
            "id": self.response_id,
            "object": "response",
            "created_at": self.created_at,
            "status": status,
            "model": self.request_model,
            "output": items,
            "usage": Value::Null,
            "incomplete_details": Value::Null,
            "error": Value::Null,
            "metadata": json!({}),
        })
    }
}

/// Format a `text/event-stream` frame: `event:` + `data:` + double LF.
fn format_sse_frame(event: &str, data: &Value) -> String {
    format!("event: {event}\ndata: {data}\n\n")
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

    // ─── unified_to_response ────────────────────────────────────────────

    use super::super::types::{UnifiedChoice, UnifiedResponse, UnifiedUsage};

    fn assistant_text(text: &str) -> UnifiedMessage {
        UnifiedMessage {
            role: Role::Assistant,
            content: vec![ContentPart::Text { text: text.into() }],
            name: None,
        }
    }

    fn build_response(
        message: UnifiedMessage,
        finish_reason: Option<FinishReason>,
        usage: Option<UnifiedUsage>,
    ) -> UnifiedResponse {
        UnifiedResponse {
            id: "chatcmpl-upstream".into(),
            model: "upstream-model".into(),
            choices: vec![UnifiedChoice {
                index: 0,
                message,
                finish_reason,
            }],
            usage,
        }
    }

    fn render(resp: &UnifiedResponse, request_model: &str) -> Value {
        ResponsesConverter
            .unified_to_response(resp, request_model)
            .expect("unified_to_response must succeed for these fixtures")
    }

    #[test]
    fn text_only_response_emits_single_message_item() {
        let resp = build_response(
            assistant_text("hello there"),
            Some(FinishReason::Stop),
            Some(UnifiedUsage {
                prompt_tokens: 5,
                completion_tokens: 3,
                total_tokens: 8,
            }),
        );
        let out = render(&resp, "gpt-4o");

        assert_eq!(out["object"], "response");
        assert!(out["id"].as_str().is_some_and(|s| s.starts_with("resp_")));
        assert_eq!(out["status"], "completed");
        assert_eq!(out["model"], "gpt-4o");
        assert!(out["created_at"].as_u64().is_some());

        let items = out["output"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["type"], "message");
        assert_eq!(items[0]["role"], "assistant");
        assert_eq!(items[0]["status"], "completed");
        assert!(items[0]["id"]
            .as_str()
            .is_some_and(|s| s.starts_with("msg_")));

        let content = items[0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["type"], "output_text");
        assert_eq!(content[0]["text"], "hello there");
        assert!(content[0]["annotations"].is_array());

        assert_eq!(out["usage"]["input_tokens"], 5);
        assert_eq!(out["usage"]["output_tokens"], 3);
        assert_eq!(out["usage"]["total_tokens"], 8);
    }

    #[test]
    fn tool_calls_become_separate_function_call_items_after_text() {
        let message = UnifiedMessage {
            role: Role::Assistant,
            content: vec![
                ContentPart::Text {
                    text: "let me check the weather".into(),
                },
                ContentPart::ToolCall(ToolCall {
                    id: "call_abc".into(),
                    name: "get_weather".into(),
                    arguments: r#"{"city":"SF"}"#.into(),
                }),
                ContentPart::ToolCall(ToolCall {
                    id: "call_def".into(),
                    name: "get_time".into(),
                    arguments: "{}".into(),
                }),
            ],
            name: None,
        };
        let resp = build_response(message, Some(FinishReason::ToolCalls), None);
        let out = render(&resp, "gpt-4o");

        assert_eq!(out["status"], "completed");
        let items = out["output"].as_array().unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0]["type"], "message");
        assert_eq!(items[1]["type"], "function_call");
        assert_eq!(items[1]["call_id"], "call_abc");
        assert_eq!(items[1]["name"], "get_weather");
        assert_eq!(items[1]["arguments"], r#"{"city":"SF"}"#);
        assert!(items[1]["id"]
            .as_str()
            .is_some_and(|s| s.starts_with("fc_")));
        assert_eq!(items[2]["type"], "function_call");
        assert_eq!(items[2]["call_id"], "call_def");
    }

    #[test]
    fn tool_only_response_skips_message_item() {
        // Some providers respond with tool calls and no narration; we
        // must not emit an empty `message` item in that case — it
        // would render as a blank assistant turn in clients.
        let message = UnifiedMessage {
            role: Role::Assistant,
            content: vec![ContentPart::ToolCall(ToolCall {
                id: "call_solo".into(),
                name: "search".into(),
                arguments: "{}".into(),
            })],
            name: None,
        };
        let resp = build_response(message, Some(FinishReason::ToolCalls), None);
        let out = render(&resp, "gpt-4o");

        let items = out["output"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["type"], "function_call");
    }

    #[test]
    fn length_finish_reason_yields_incomplete_status() {
        let resp = build_response(assistant_text("partial"), Some(FinishReason::Length), None);
        let out = render(&resp, "gpt-4o");
        assert_eq!(out["status"], "incomplete");
        assert_eq!(out["incomplete_details"]["reason"], "max_output_tokens");
    }

    #[test]
    fn content_filter_finish_reason_yields_incomplete_status() {
        let resp = build_response(
            assistant_text("[redacted]"),
            Some(FinishReason::ContentFilter),
            None,
        );
        let out = render(&resp, "gpt-4o");
        assert_eq!(out["status"], "incomplete");
        assert_eq!(out["incomplete_details"]["reason"], "content_filter");
    }

    #[test]
    fn empty_choices_collapse_to_an_empty_output_array() {
        let resp = UnifiedResponse {
            id: "chatcmpl-empty".into(),
            model: "upstream-model".into(),
            choices: vec![],
            usage: None,
        };
        let out = render(&resp, "gpt-4o");
        assert_eq!(out["status"], "completed");
        assert_eq!(out["output"].as_array().map(Vec::len), Some(0));
        assert_eq!(out["usage"]["input_tokens"], 0);
        assert_eq!(out["usage"]["total_tokens"], 0);
    }

    #[test]
    fn request_model_overrides_upstream_model_field() {
        // Clients correlate request and response by `model`; when an
        // upstream account-pool rewrites the model name we still want
        // the originating request's model to come back.
        let resp = build_response(assistant_text("ok"), Some(FinishReason::Stop), None);
        let out = render(&resp, "gpt-4o-mini");
        assert_eq!(out["model"], "gpt-4o-mini");
    }

    #[test]
    fn empty_request_model_falls_back_to_upstream_model() {
        // Should never happen in practice — handler always sends the
        // body's `model` — but a robust serialiser shouldn't blank
        // out the field if the caller forgets.
        let resp = build_response(assistant_text("ok"), Some(FinishReason::Stop), None);
        let out = render(&resp, "");
        assert_eq!(out["model"], "upstream-model");
    }

    // ─── ResponsesStreamEncoder ─────────────────────────────────────────

    use super::super::types::{StreamChoice, StreamDelta, StreamToolCall, UnifiedStreamChunk};

    /// Parse one SSE frame ("event: NAME\ndata: JSON\n\n") into
    /// `(event, data_json)` so tests can assert on structure instead
    /// of raw strings.
    fn parse_sse_frame(frame: &str) -> (String, Value) {
        assert!(
            frame.ends_with("\n\n"),
            "frame must terminate with double LF, got: {frame:?}"
        );
        let mut event = String::new();
        let mut data = String::new();
        for line in frame.trim_end_matches('\n').lines() {
            if let Some(rest) = line.strip_prefix("event: ") {
                event = rest.to_string();
            } else if let Some(rest) = line.strip_prefix("data: ") {
                data = rest.to_string();
            }
        }
        let parsed: Value = serde_json::from_str(&data).expect("data line must be valid JSON");
        (event, parsed)
    }

    fn event_names(frames: &[String]) -> Vec<String> {
        frames.iter().map(|f| parse_sse_frame(f).0).collect()
    }

    fn text_chunk(content: &str, finish_reason: Option<FinishReason>) -> UnifiedStreamChunk {
        UnifiedStreamChunk {
            id: "chatcmpl-x".into(),
            model: "upstream".into(),
            choices: vec![StreamChoice {
                index: 0,
                delta: Some(StreamDelta {
                    role: None,
                    content: Some(content.into()),
                    tool_calls: Vec::new(),
                }),
                finish_reason,
            }],
            usage: None,
        }
    }

    fn tool_chunk(
        index: u32,
        id: Option<&str>,
        name: Option<&str>,
        arguments: Option<&str>,
        finish_reason: Option<FinishReason>,
    ) -> UnifiedStreamChunk {
        UnifiedStreamChunk {
            id: "chatcmpl-x".into(),
            model: "upstream".into(),
            choices: vec![StreamChoice {
                index: 0,
                delta: Some(StreamDelta {
                    role: None,
                    content: None,
                    tool_calls: vec![StreamToolCall {
                        index,
                        id: id.map(str::to_owned),
                        name: name.map(str::to_owned),
                        arguments: arguments.map(str::to_owned),
                    }],
                }),
                finish_reason,
            }],
            usage: None,
        }
    }

    #[test]
    fn start_frames_emit_created_then_in_progress_exactly_once() {
        let mut enc = ResponsesStreamEncoder::new("gpt-4o".into());
        let first = enc.start_frames();
        assert_eq!(
            event_names(&first),
            vec!["response.created", "response.in_progress"]
        );
        // Idempotent — calling again returns nothing.
        assert!(enc.start_frames().is_empty());
    }

    #[test]
    fn text_only_stream_produces_full_event_lifecycle() {
        let mut enc = ResponsesStreamEncoder::new("gpt-4o".into());
        let mut frames = enc.start_frames();
        frames.extend(enc.process_chunk(&text_chunk("Hel", None)));
        frames.extend(enc.process_chunk(&text_chunk("lo", None)));
        frames.extend(enc.process_chunk(&text_chunk("!", Some(FinishReason::Stop))));
        frames.extend(enc.finalize(None));

        let names = event_names(&frames);
        assert_eq!(
            names,
            vec![
                "response.created",
                "response.in_progress",
                "response.output_item.added",
                "response.content_part.added",
                "response.output_text.delta",
                "response.output_text.delta",
                "response.output_text.delta",
                "response.output_text.done",
                "response.content_part.done",
                "response.output_item.done",
                "response.completed",
            ]
        );

        // `output_text.done` must carry the concatenated text, and the
        // final completed snapshot must surface the full output array
        // with status=completed.
        let done = parse_sse_frame(&frames[7]).1;
        assert_eq!(done["text"], "Hello!");
        let completed = parse_sse_frame(&frames[10]).1;
        assert_eq!(completed["response"]["status"], "completed");
        let output = completed["response"]["output"].as_array().unwrap();
        assert_eq!(output.len(), 1);
        assert_eq!(
            output[0]["content"][0]["text"], "Hello!",
            "completed snapshot must include the accumulated text"
        );
    }

    #[test]
    fn single_tool_call_drives_function_call_lifecycle() {
        let mut enc = ResponsesStreamEncoder::new("gpt-4o".into());
        let _ = enc.start_frames();
        let mut frames: Vec<String> = Vec::new();
        frames.extend(enc.process_chunk(&tool_chunk(
            0,
            Some("call_w"),
            Some("get_weather"),
            Some("{"),
            None,
        )));
        frames.extend(enc.process_chunk(&tool_chunk(0, None, None, Some("\"city\":\"SF\""), None)));
        frames.extend(enc.process_chunk(&tool_chunk(
            0,
            None,
            None,
            Some("}"),
            Some(FinishReason::ToolCalls),
        )));
        frames.extend(enc.finalize(None));

        let names = event_names(&frames);
        assert_eq!(
            names,
            vec![
                "response.output_item.added",
                "response.function_call_arguments.delta",
                "response.function_call_arguments.delta",
                "response.function_call_arguments.delta",
                "response.function_call_arguments.done",
                "response.output_item.done",
                "response.completed",
            ]
        );
        let done = parse_sse_frame(&frames[4]).1;
        assert_eq!(done["arguments"], r#"{"city":"SF"}"#);
        let completed = parse_sse_frame(&frames[6]).1;
        let item = &completed["response"]["output"][0];
        assert_eq!(item["type"], "function_call");
        assert_eq!(item["call_id"], "call_w");
        assert_eq!(item["name"], "get_weather");
        assert_eq!(item["arguments"], r#"{"city":"SF"}"#);
    }

    #[test]
    fn text_then_tool_call_closes_message_before_opening_call() {
        let mut enc = ResponsesStreamEncoder::new("gpt-4o".into());
        let _ = enc.start_frames();
        let mut frames: Vec<String> = Vec::new();
        frames.extend(enc.process_chunk(&text_chunk("checking", None)));
        frames.extend(enc.process_chunk(&tool_chunk(
            0,
            Some("call_z"),
            Some("lookup"),
            Some("{}"),
            Some(FinishReason::ToolCalls),
        )));
        frames.extend(enc.finalize(None));

        let names = event_names(&frames);
        // Message lifecycle closes (output_text.done / content_part.done
        // / output_item.done) before the function_call lifecycle opens.
        let message_done = names
            .iter()
            .position(|e| e == "response.output_item.done")
            .expect("message must close");
        let tool_open = names
            .iter()
            .skip(message_done + 1)
            .position(|e| e == "response.output_item.added")
            .map(|i| i + message_done + 1)
            .expect("function_call must open after");
        let tool_done = names
            .iter()
            .skip(tool_open + 1)
            .position(|e| e == "response.output_item.done")
            .map(|i| i + tool_open + 1)
            .expect("function_call must close");
        assert!(message_done < tool_open && tool_open < tool_done);
    }

    #[test]
    fn second_tool_call_with_new_id_opens_a_new_item() {
        let mut enc = ResponsesStreamEncoder::new("gpt-4o".into());
        let _ = enc.start_frames();
        let mut frames: Vec<String> = Vec::new();
        frames.extend(enc.process_chunk(&tool_chunk(
            0,
            Some("call_a"),
            Some("alpha"),
            Some("{}"),
            None,
        )));
        frames.extend(enc.process_chunk(&tool_chunk(
            1,
            Some("call_b"),
            Some("beta"),
            Some("{}"),
            Some(FinishReason::ToolCalls),
        )));
        frames.extend(enc.finalize(None));

        let completed = frames
            .iter()
            .map(|f| parse_sse_frame(f))
            .find(|(ev, _)| ev == "response.completed")
            .map(|(_, data)| data)
            .expect("completed must be emitted");
        let output = completed["response"]["output"].as_array().unwrap();
        assert_eq!(output.len(), 2);
        assert_eq!(output[0]["call_id"], "call_a");
        assert_eq!(output[1]["call_id"], "call_b");
    }

    #[test]
    fn upstream_error_emits_error_event_and_closes_open_item() {
        let mut enc = ResponsesStreamEncoder::new("gpt-4o".into());
        let _ = enc.start_frames();
        let mut frames: Vec<String> = Vec::new();
        frames.extend(enc.process_chunk(&text_chunk("partial...", None)));
        frames.extend(enc.error_frame("upstream timed out"));

        let names = event_names(&frames);
        // Open text item must close before `error` is emitted, so the
        // client never sees a dangling in_progress item.
        let close_idx = names
            .iter()
            .position(|e| e == "response.output_item.done")
            .expect("open text item must close before error");
        let error_idx = names
            .iter()
            .position(|e| e == "error")
            .expect("error event must be emitted");
        assert!(close_idx < error_idx);

        let error_data = parse_sse_frame(&frames[error_idx]).1;
        assert_eq!(error_data["code"], "upstream_error");
        assert_eq!(error_data["message"], "upstream timed out");
    }

    #[test]
    fn length_finish_reason_marks_completed_snapshot_as_incomplete() {
        let mut enc = ResponsesStreamEncoder::new("gpt-4o".into());
        let _ = enc.start_frames();
        let _ = enc.process_chunk(&text_chunk("truncated", Some(FinishReason::Length)));
        let final_frames = enc.finalize(None);
        let completed = parse_sse_frame(final_frames.last().unwrap()).1;
        assert_eq!(completed["response"]["status"], "incomplete");
        assert_eq!(
            completed["response"]["incomplete_details"]["reason"],
            "max_output_tokens"
        );
    }

    #[test]
    fn empty_stream_finalize_emits_completed_with_empty_output() {
        let mut enc = ResponsesStreamEncoder::new("gpt-4o".into());
        let mut frames = enc.start_frames();
        frames.extend(enc.finalize(None));
        let names = event_names(&frames);
        assert_eq!(
            names,
            vec![
                "response.created",
                "response.in_progress",
                "response.completed",
            ]
        );
        let completed = parse_sse_frame(frames.last().unwrap()).1;
        assert_eq!(
            completed["response"]["output"].as_array().map(Vec::len),
            Some(0),
        );
    }

    #[test]
    fn finalize_includes_usage_when_provided() {
        let mut enc = ResponsesStreamEncoder::new("gpt-4o".into());
        let _ = enc.start_frames();
        let _ = enc.process_chunk(&text_chunk("hi", Some(FinishReason::Stop)));
        let usage = UnifiedUsage {
            prompt_tokens: 11,
            completion_tokens: 4,
            total_tokens: 15,
        };
        let final_frames = enc.finalize(Some(&usage));
        let completed = parse_sse_frame(final_frames.last().unwrap()).1;
        assert_eq!(completed["response"]["usage"]["input_tokens"], 11);
        assert_eq!(completed["response"]["usage"]["output_tokens"], 4);
        assert_eq!(completed["response"]["usage"]["total_tokens"], 15);
    }

    #[test]
    fn every_frame_terminates_with_double_linefeed() {
        // SSE clients are pretty strict about frame boundaries. Regress
        // any helper accidentally emitting a single `\n`.
        let mut enc = ResponsesStreamEncoder::new("gpt-4o".into());
        let mut frames = enc.start_frames();
        frames.extend(enc.process_chunk(&text_chunk("hi", Some(FinishReason::Stop))));
        frames.extend(enc.finalize(None));
        for frame in &frames {
            assert!(
                frame.ends_with("\n\n"),
                "missing double LF on frame: {frame:?}"
            );
        }
    }
}
