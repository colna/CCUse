//! Integration tests for Anthropic Messages-compatible providers.
//!
//! These tests pin the cc-switch-aligned relay behavior: Claude relay
//! endpoints receive `/v1/messages`, Bearer auth by default, full
//! Anthropic headers, and streaming probes treat the first SSE chunk as
//! success.

use ccuse_desktop_lib::providers::api::{ApiRequest, ChatMessage, HealthStatus, Provider};
use ccuse_desktop_lib::providers::{AnthropicProvider, ProviderError};
use futures::StreamExt;
use serde_json::Value;
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn sample_request(stream: bool) -> ApiRequest {
    ApiRequest {
        model: "claude-opus-4-6".into(),
        messages: vec![
            ChatMessage {
                role: "system".into(),
                content: "You are terse.".into(),
                tool_call_id: None,
                tool_calls: vec![],
            },
            ChatMessage {
                role: "user".into(),
                content: "ping".into(),
                tool_call_id: None,
                tool_calls: vec![],
            },
        ],
        temperature: Some(0.7),
        max_tokens: Some(64),
        stream,
        tools: vec![],
    }
}

fn anthropic_response_body() -> Value {
    serde_json::json!({
        "id": "msg_test",
        "type": "message",
        "role": "assistant",
        "model": "claude-opus-4-6",
        "content": [{"type": "text", "text": "pong"}],
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 4, "output_tokens": 1}
    })
}

#[tokio::test]
async fn send_request_uses_messages_endpoint_and_bearer_auth_for_relays() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/messages"))
        .and(header("authorization", "Bearer sk-relay"))
        .and(header("anthropic-version", "2023-06-01"))
        .respond_with(ResponseTemplate::new(200).set_body_json(anthropic_response_body()))
        .expect(1)
        .mount(&server)
        .await;

    let provider =
        AnthropicProvider::new("p1", "Relay", format!("{}/api", server.uri()), "sk-relay")
            .expect("build provider");
    let response = provider
        .send_request(sample_request(true))
        .await
        .expect("ok");

    assert_eq!(response.id, "msg_test");
    assert_eq!(response.model, "claude-opus-4-6");
    assert_eq!(response.choices[0].message.content, "pong");
    assert_eq!(response.choices[0].finish_reason.as_deref(), Some("stop"));
    assert_eq!(response.usage.expect("usage").total_tokens, 5);

    let received = server.received_requests().await.expect("received");
    let body: Value = serde_json::from_slice(&received[0].body).expect("json");
    assert_eq!(body["model"], "claude-opus-4-6");
    assert_eq!(body["system"], "You are terse.");
    assert!(
        body.get("stream").is_none(),
        "non-streaming request omits stream"
    );
    assert_eq!(body["messages"][0]["role"], "user");
    assert_eq!(body["messages"][0]["content"][0]["text"], "ping");
}

#[tokio::test]
async fn upstream_401_maps_to_unauthorized_and_is_retriable() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(401).set_body_string("invalid api key"))
        .expect(1)
        .mount(&server)
        .await;

    let provider =
        AnthropicProvider::new("p1", "Relay", server.uri(), "sk-relay").expect("build provider");
    let err = provider
        .send_request(sample_request(false))
        .await
        .expect_err("must fail");

    assert!(matches!(err, ProviderError::Unauthorized(_)));
    assert!(err.is_retriable());
}

#[tokio::test]
async fn health_check_uses_streaming_messages_probe_and_first_chunk_is_healthy() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("authorization", "Bearer sk-relay"))
        .and(header("accept", "text/event-stream"))
        .and(header("accept-encoding", "identity"))
        .and(body_partial_json(serde_json::json!({
            "model": "claude-haiku-4-5-20251001",
            "stream": true,
            "max_tokens": 1,
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"model\":\"claude-haiku-4-5-20251001\"}}\n\n")
                .insert_header("content-type", "text/event-stream"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let provider =
        AnthropicProvider::new("p1", "Relay", server.uri(), "sk-relay").expect("build provider");
    let status = provider.health_check().await.expect("ok");

    assert_eq!(status, HealthStatus::Healthy);
}

#[tokio::test]
async fn send_stream_request_converts_anthropic_sse_to_openai_sse() {
    let server = MockServer::start().await;
    let sse = r#"event: message_start
data: {"type":"message_start","message":{"id":"msg_stream","type":"message","role":"assistant","model":"claude-opus-4-6","content":[]}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hel"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"lo"}}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":2}}

event: message_stop
data: {"type":"message_stop"}

"#;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(sse)
                .insert_header("content-type", "text/event-stream"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let provider =
        AnthropicProvider::new("p1", "Relay", server.uri(), "sk-relay").expect("build provider");
    let mut stream = provider
        .send_stream_request(sample_request(false))
        .await
        .expect("stream");
    let mut body = String::new();
    while let Some(chunk) = stream.next().await {
        body.push_str(std::str::from_utf8(&chunk.expect("chunk")).expect("utf8"));
    }

    assert!(body.contains("data: [DONE]"));
    assert!(body.contains("\"content\":\"Hel\""));
    assert!(body.contains("\"content\":\"lo\""));
    assert!(body.contains("\"finish_reason\":\"stop\""));

    let received = server.received_requests().await.expect("received");
    let upstream_body: Value = serde_json::from_slice(&received[0].body).expect("json");
    assert_eq!(upstream_body["stream"], true);
}
