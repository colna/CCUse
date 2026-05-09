//! Integration tests for `OpenAIProvider` against a `wiremock` upstream.
//!
//! Pin the contract clients depend on:
//! 1. successful chat-completions decodes into `ApiResponse`,
//! 2. `Authorization: Bearer <key>` is forwarded verbatim,
//! 3. `stream` is forced to `false` even if the caller set `true`,
//! 4. explicit model names are preserved and missing models use provider defaults,
//! 5. 401 / 429 / 500 / 400 land in the right `ProviderError` variant,
//! 6. `health_check` uses the shared cc-switch style stream probe,
//!    while `list_models` still reads `GET /v1/models`.

use ccuse_desktop_lib::providers::api::ProviderError;
use ccuse_desktop_lib::providers::api::{
    ApiRequest, ChatContent, ChatContentPart, ChatImageUrl, ChatMessage, HealthStatus, Provider,
};
use ccuse_desktop_lib::providers::default_models::OPENAI_DEFAULT_MODELS;
use ccuse_desktop_lib::providers::OpenAIProvider;
use futures::StreamExt;
use serde_json::Value;
use wiremock::matchers::{body_json, body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn sample_request(stream: bool) -> ApiRequest {
    ApiRequest {
        model: "gpt-5.4".into(),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: "ping".into(),
            tool_call_id: None,
            tool_calls: vec![],
        }],
        temperature: Some(0.7),
        max_tokens: Some(64),
        stream,
        tools: vec![],
    }
}

fn sample_request_without_model(stream: bool) -> ApiRequest {
    let mut request = sample_request(stream);
    request.model.clear();
    request
}

fn multimodal_request(stream: bool) -> ApiRequest {
    ApiRequest {
        model: "gpt-5.4".into(),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: ChatContent::parts(vec![
                ChatContentPart::Text {
                    text: "describe this".into(),
                },
                ChatContentPart::ImageUrl {
                    image_url: ChatImageUrl {
                        url: "data:image/png;base64,abc123".into(),
                        detail: Some("high".into()),
                    },
                },
            ]),
            tool_call_id: None,
            tool_calls: vec![],
        }],
        temperature: None,
        max_tokens: None,
        stream,
        tools: vec![],
    }
}

fn fixture_response_body() -> Value {
    serde_json::json!({
        "id": "chatcmpl-test",
        "object": "chat.completion",
        "created": 1_700_000_000_u64,
        "model": "gpt-5.4",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "pong"},
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 4, "completion_tokens": 1, "total_tokens": 5}
    })
}

#[tokio::test]
async fn send_request_round_trips_a_successful_completion() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("authorization", "Bearer sk-test"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture_response_body()))
        .mount(&server)
        .await;

    let provider =
        OpenAIProvider::new("p1", "Mock", server.uri(), "sk-test").expect("build provider");
    let response = provider
        .send_request(sample_request(false))
        .await
        .expect("ok");
    assert_eq!(response.id, "chatcmpl-test");
    assert_eq!(response.model, "gpt-5.4");
    assert_eq!(response.choices.len(), 1);
    assert_eq!(response.choices[0].message.content, "pong");
    assert_eq!(response.usage.expect("usage").total_tokens, 5);
}

#[tokio::test]
async fn send_request_preserves_multimodal_content_on_the_wire() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture_response_body()))
        .mount(&server)
        .await;

    let provider =
        OpenAIProvider::new("p1", "Mock", server.uri(), "sk-test").expect("build provider");
    provider
        .send_request(multimodal_request(false))
        .await
        .expect("ok");

    let received = &server.received_requests().await.expect("requests")[0];
    let body: Value = serde_json::from_slice(&received.body).expect("json");
    assert_eq!(body["messages"][0]["content"][0]["type"], "text");
    assert_eq!(
        body["messages"][0]["content"][1]["image_url"]["url"],
        "data:image/png;base64,abc123",
    );
    assert_eq!(
        body["messages"][0]["content"][1]["image_url"]["detail"],
        "high",
    );
}

#[tokio::test]
async fn send_request_forces_stream_false_on_the_wire() {
    let server = MockServer::start().await;
    // Inspect the captured request body to verify `stream` was
    // overridden, even though the caller asked for streaming.
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture_response_body()))
        .mount(&server)
        .await;

    let provider =
        OpenAIProvider::new("p1", "Mock", server.uri(), "sk-test").expect("build provider");
    provider
        .send_request(sample_request(true))
        .await
        .expect("ok");

    let received = &server.received_requests().await.expect("requests")[0];
    let body: Value = serde_json::from_slice(&received.body).expect("json");
    assert_eq!(body["stream"], serde_json::json!(false));
    assert_eq!(body["model"], "gpt-5.4");
}

#[tokio::test]
async fn send_request_uses_default_model_when_request_model_is_empty() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture_response_body()))
        .mount(&server)
        .await;

    let provider =
        OpenAIProvider::new("p1", "Mock", server.uri(), "sk-test").expect("build provider");
    provider
        .send_request(sample_request_without_model(false))
        .await
        .expect("ok");

    let received = &server.received_requests().await.expect("requests")[0];
    let body: Value = serde_json::from_slice(&received.body).expect("json");
    assert_eq!(body["model"], OPENAI_DEFAULT_MODELS[0]);
}

#[tokio::test]
async fn send_request_tries_next_default_model_when_first_model_is_unavailable() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_partial_json(serde_json::json!({"model": "gpt-5.5"})))
        .respond_with(
            ResponseTemplate::new(400)
                .set_body_string(r#"{"error":{"message":"The model `gpt-5.5` does not exist"}}"#),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_partial_json(serde_json::json!({"model": "gpt-5.4"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture_response_body()))
        .expect(1)
        .mount(&server)
        .await;

    let provider = OpenAIProvider::with_default_models(
        "p1",
        "Mock",
        server.uri(),
        "sk-test",
        vec!["gpt-5.5".to_owned(), "gpt-5.4".to_owned()],
    )
    .expect("build provider");
    provider
        .send_request(sample_request_without_model(false))
        .await
        .expect("ok");

    let received = server.received_requests().await.expect("requests");
    let models = received
        .iter()
        .map(|request| {
            serde_json::from_slice::<Value>(&request.body).expect("json")["model"]
                .as_str()
                .expect("model")
                .to_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(models, vec!["gpt-5.5", "gpt-5.4"]);
}

#[tokio::test]
async fn upstream_401_maps_to_unauthorized_and_is_not_retriable() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(401).set_body_string("invalid api key"))
        .mount(&server)
        .await;
    let provider =
        OpenAIProvider::new("p1", "Mock", server.uri(), "sk-test").expect("build provider");
    let err = provider
        .send_request(sample_request(false))
        .await
        .expect_err("must fail");
    assert!(matches!(err, ProviderError::Unauthorized(_)));
    assert!(!err.is_retriable());
}

#[tokio::test]
async fn upstream_429_maps_to_rate_limited_and_is_retriable() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(429).set_body_string("slow down"))
        .mount(&server)
        .await;
    let provider =
        OpenAIProvider::new("p1", "Mock", server.uri(), "sk-test").expect("build provider");
    let err = provider
        .send_request(sample_request(false))
        .await
        .expect_err("must fail");
    assert!(matches!(err, ProviderError::RateLimited(_)));
    assert!(err.is_retriable());
}

#[tokio::test]
async fn upstream_500_maps_to_upstream_with_correct_status() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(503).set_body_string("upstream gone"))
        .mount(&server)
        .await;
    let provider =
        OpenAIProvider::new("p1", "Mock", server.uri(), "sk-test").expect("build provider");
    let err = provider
        .send_request(sample_request(false))
        .await
        .expect_err("must fail");
    match err {
        ProviderError::Upstream { status, body } => {
            assert_eq!(status, 503);
            assert!(body.contains("upstream gone"));
        }
        other => panic!("expected Upstream, got {other:?}"),
    }
}

#[tokio::test]
async fn upstream_400_maps_to_bad_request_and_is_not_retriable() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(400).set_body_string("model unknown"))
        .mount(&server)
        .await;
    let provider =
        OpenAIProvider::new("p1", "Mock", server.uri(), "sk-test").expect("build provider");
    let err = provider
        .send_request(sample_request(false))
        .await
        .expect_err("must fail");
    assert!(matches!(err, ProviderError::BadRequest(_)));
    assert!(!err.is_retriable());
}

#[tokio::test]
async fn malformed_success_body_yields_decode_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;
    let provider =
        OpenAIProvider::new("p1", "Mock", server.uri(), "sk-test").expect("build provider");
    let err = provider
        .send_request(sample_request(false))
        .await
        .expect_err("must fail");
    assert!(matches!(err, ProviderError::Decode(_)));
}

#[tokio::test]
async fn health_check_uses_stream_probe_and_maps_success_status() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("authorization", "Bearer sk-test"))
        .and(body_json(serde_json::json!({
            "model": "gpt-5.4",
            "messages": [{ "role": "user", "content": "Who are you?" }],
            "max_tokens": 1,
            "stream": true,
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\n")
                .insert_header("content-type", "text/event-stream"),
        )
        .expect(1)
        .mount(&server)
        .await;
    let provider =
        OpenAIProvider::new("p1", "Mock", server.uri(), "sk-test").expect("build provider");
    let status = provider.health_check().await.expect("ok");
    assert_eq!(status, HealthStatus::Healthy);
}

#[tokio::test]
async fn list_models_calls_v1_models_and_decodes_data() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .and(header("authorization", "Bearer sk-test"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "object": "list",
            "data": [
                {"id": "gpt-5.4", "object": "model", "owned_by": "openai"},
                {"id": "gpt-5.4"}
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;
    let provider =
        OpenAIProvider::new("p1", "Mock", server.uri(), "sk-test").expect("build provider");

    let models = provider.list_models().await.expect("models");

    assert_eq!(models.len(), 2);
    assert_eq!(models[0].id, "gpt-5.4");
    assert_eq!(models[0].owned_by.as_deref(), Some("openai"));
    assert_eq!(models[1].id, "gpt-5.4");
    assert_eq!(models[1].object, "model");
}

/// Concatenate every chunk into a single byte buffer. Surfaces the
/// first transport error.
async fn drain_stream(
    mut stream: ccuse_desktop_lib::providers::api::StreamingResponse,
) -> Result<Vec<u8>, ProviderError> {
    let mut out = Vec::new();
    while let Some(chunk) = stream.next().await {
        out.extend_from_slice(&chunk?);
    }
    Ok(out)
}

#[tokio::test]
async fn streaming_request_forwards_sse_chunks_verbatim() {
    let server = MockServer::start().await;
    let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"Hel\"}}]}\n\n\
               data: {\"choices\":[{\"delta\":{\"content\":\"lo\"}}]}\n\n\
               data: [DONE]\n\n";
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("authorization", "Bearer sk-test"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(sse)
                .insert_header("content-type", "text/event-stream"),
        )
        .mount(&server)
        .await;

    let provider =
        OpenAIProvider::new("p1", "Mock", server.uri(), "sk-test").expect("build provider");
    let stream = provider
        .send_stream_request(sample_request(true))
        .await
        .expect("stream ok");
    let body = drain_stream(stream).await.expect("drain ok");
    let text = String::from_utf8(body).expect("utf8");
    assert!(text.contains("data: [DONE]"));
    assert!(text.contains("\"Hel\""));
    assert!(text.contains("\"lo\""));
}

#[tokio::test]
async fn streaming_request_forces_stream_true_on_the_wire() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("data: [DONE]\n\n")
                .insert_header("content-type", "text/event-stream"),
        )
        .mount(&server)
        .await;

    let provider =
        OpenAIProvider::new("p1", "Mock", server.uri(), "sk-test").expect("build provider");
    // Caller passes `stream: false` — the provider must override it.
    let stream = provider
        .send_stream_request(sample_request(false))
        .await
        .expect("stream ok");
    drop(stream);

    let received = &server.received_requests().await.expect("requests")[0];
    let body: Value = serde_json::from_slice(&received.body).expect("json");
    assert_eq!(body["stream"], serde_json::json!(true));
}

#[tokio::test]
async fn streaming_request_preserves_multimodal_content_on_the_wire() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("data: [DONE]\n\n")
                .insert_header("content-type", "text/event-stream"),
        )
        .mount(&server)
        .await;

    let provider =
        OpenAIProvider::new("p1", "Mock", server.uri(), "sk-test").expect("build provider");
    let stream = provider
        .send_stream_request(multimodal_request(true))
        .await
        .expect("stream ok");
    drop(stream);

    let received = &server.received_requests().await.expect("requests")[0];
    let body: Value = serde_json::from_slice(&received.body).expect("json");
    assert_eq!(
        body["messages"][0]["content"][1]["image_url"]["url"],
        "data:image/png;base64,abc123",
    );
    assert_eq!(body["stream"], serde_json::json!(true));
}

#[tokio::test]
async fn streaming_401_yields_unauthorized_before_any_chunks() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(401).set_body_string("bad key"))
        .mount(&server)
        .await;

    let provider =
        OpenAIProvider::new("p1", "Mock", server.uri(), "sk-test").expect("build provider");
    let result = provider.send_stream_request(sample_request(true)).await;
    let Err(err) = result else {
        panic!("must fail before stream returns");
    };
    assert!(matches!(err, ProviderError::Unauthorized(_)));
}

#[tokio::test]
async fn health_check_reports_down_on_429_stream_probe_failure() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&server)
        .await;
    let provider =
        OpenAIProvider::new("p1", "Mock", server.uri(), "sk-test").expect("build provider");
    let status = provider.health_check().await.expect("ok");
    assert_eq!(status, HealthStatus::Down);
}
