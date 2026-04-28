//! Integration tests for `OpenAIProvider` against a `wiremock` upstream.
//!
//! Pin the contract clients depend on:
//! 1. successful chat-completions decodes into `ApiResponse`,
//! 2. `Authorization: Bearer <key>` is forwarded verbatim,
//! 3. `stream` is forced to `false` even if the caller set `true`,
//! 4. 401 / 429 / 500 / 400 land in the right `ProviderError` variant,
//! 5. `health_check` reads `GET /v1/models`.

use ccuse_desktop_lib::providers::api::ProviderError;
use ccuse_desktop_lib::providers::api::{ApiRequest, ChatMessage, HealthStatus, Provider};
use ccuse_desktop_lib::providers::OpenAIProvider;
use serde_json::Value;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn sample_request(stream: bool) -> ApiRequest {
    ApiRequest {
        model: "gpt-4o".into(),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: "ping".into(),
        }],
        temperature: Some(0.7),
        max_tokens: Some(64),
        stream,
    }
}

fn fixture_response_body() -> Value {
    serde_json::json!({
        "id": "chatcmpl-test",
        "object": "chat.completion",
        "created": 1_700_000_000_u64,
        "model": "gpt-4o",
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
    assert_eq!(response.model, "gpt-4o");
    assert_eq!(response.choices.len(), 1);
    assert_eq!(response.choices[0].message.content, "pong");
    assert_eq!(response.usage.expect("usage").total_tokens, 5);
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
    assert_eq!(body["model"], serde_json::json!("gpt-4o"));
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
async fn health_check_calls_v1_models_and_maps_status() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": []})))
        .mount(&server)
        .await;
    let provider =
        OpenAIProvider::new("p1", "Mock", server.uri(), "sk-test").expect("build provider");
    let status = provider.health_check().await.expect("ok");
    assert_eq!(status, HealthStatus::Healthy);
}

#[tokio::test]
async fn health_check_reports_degraded_on_429() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&server)
        .await;
    let provider =
        OpenAIProvider::new("p1", "Mock", server.uri(), "sk-test").expect("build provider");
    let status = provider.health_check().await.expect("ok");
    assert_eq!(status, HealthStatus::Degraded);
}
