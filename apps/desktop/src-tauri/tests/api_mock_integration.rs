//! T1.0.5.08 — Three-provider API mock integration tests.
//! T1.0.5.09 — Fault injection tests (429, 5xx, timeout, connection refused).
//!
//! Uses `wiremock` to stand up mock HTTP servers that simulate `OpenAI`,
//! Anthropic, and Gemini upstream endpoints.  Each test verifies the
//! wire format returned by the mock and, for fault injection, that
//! errors are properly surfaced (not silently swallowed).

use std::time::Duration;

use reqwest::Client;
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ═══════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════

fn openai_chat_request_body() -> Value {
    json!({
        "model": "gpt-5.4",
        "messages": [{"role": "user", "content": "ping"}],
        "temperature": 0.7,
        "max_tokens": 64,
        "stream": false
    })
}

fn anthropic_messages_request_body() -> Value {
    json!({
        "model": "claude-sonnet-4-6",
        "max_tokens": 1024,
        "messages": [{"role": "user", "content": "ping"}]
    })
}

fn gemini_generate_content_request_body() -> Value {
    json!({
        "contents": [{"role": "user", "parts": [{"text": "ping"}]}],
        "generationConfig": {"temperature": 0.7, "maxOutputTokens": 64}
    })
}

fn openai_success_response() -> Value {
    json!({
        "id": "chatcmpl-mock",
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

fn anthropic_success_response() -> Value {
    json!({
        "id": "msg_mock",
        "type": "message",
        "role": "assistant",
        "model": "claude-sonnet-4-6",
        "content": [{"type": "text", "text": "pong"}],
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 4, "output_tokens": 1}
    })
}

fn gemini_success_response() -> Value {
    json!({
        "candidates": [{
            "content": {
                "role": "model",
                "parts": [{"text": "pong"}]
            },
            "finishReason": "STOP"
        }],
        "usageMetadata": {
            "promptTokenCount": 4,
            "candidatesTokenCount": 1,
            "totalTokenCount": 5
        }
    })
}

/// Build a short-timeout client for fault injection tests where we
/// don't want to wait 600 s for the default provider timeout.
fn short_timeout_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .expect("build reqwest client")
}

// ═══════════════════════════════════════════════════════════════════
// T1.0.5.08 — Three-provider mock integration tests
// ═══════════════════════════════════════════════════════════════════

mod three_provider_mocks {
    use super::*;

    #[tokio::test]
    async fn openai_mock_returns_valid_chat_completion() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(openai_success_response()))
            .expect(1)
            .mount(&server)
            .await;

        let client = Client::new();
        let resp = client
            .post(format!("{}/v1/chat/completions", server.uri()))
            .json(&openai_chat_request_body())
            .send()
            .await
            .expect("request should succeed");

        assert_eq!(resp.status(), 200);
        let body: Value = resp.json().await.expect("json");
        assert_eq!(body["id"], "chatcmpl-mock");
        assert_eq!(body["model"], "gpt-5.4");
        assert_eq!(body["choices"][0]["message"]["role"], "assistant");
        assert_eq!(body["choices"][0]["message"]["content"], "pong");
        assert_eq!(body["choices"][0]["finish_reason"], "stop");
        assert_eq!(body["usage"]["total_tokens"], 5);
    }

    #[tokio::test]
    async fn anthropic_mock_returns_valid_messages_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(anthropic_success_response()))
            .expect(1)
            .mount(&server)
            .await;

        let client = Client::new();
        let resp = client
            .post(format!("{}/v1/messages", server.uri()))
            .json(&anthropic_messages_request_body())
            .send()
            .await
            .expect("request should succeed");

        assert_eq!(resp.status(), 200);
        let body: Value = resp.json().await.expect("json");
        assert_eq!(body["id"], "msg_mock");
        assert_eq!(body["type"], "message");
        assert_eq!(body["role"], "assistant");
        assert_eq!(body["model"], "claude-sonnet-4-6");
        assert_eq!(body["content"][0]["type"], "text");
        assert_eq!(body["content"][0]["text"], "pong");
        assert_eq!(body["stop_reason"], "end_turn");
        assert_eq!(body["usage"]["input_tokens"], 4);
        assert_eq!(body["usage"]["output_tokens"], 1);
    }

    #[tokio::test]
    async fn gemini_mock_returns_valid_generate_content_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1beta/models/gemini-pro:generateContent"))
            .respond_with(ResponseTemplate::new(200).set_body_json(gemini_success_response()))
            .expect(1)
            .mount(&server)
            .await;

        let client = Client::new();
        let resp = client
            .post(format!(
                "{}/v1beta/models/gemini-pro:generateContent",
                server.uri()
            ))
            .json(&gemini_generate_content_request_body())
            .send()
            .await
            .expect("request should succeed");

        assert_eq!(resp.status(), 200);
        let body: Value = resp.json().await.expect("json");
        assert_eq!(body["candidates"][0]["content"]["role"], "model");
        assert_eq!(body["candidates"][0]["content"]["parts"][0]["text"], "pong");
        assert_eq!(body["candidates"][0]["finishReason"], "STOP");
        assert_eq!(body["usageMetadata"]["totalTokenCount"], 5);
    }

    #[tokio::test]
    async fn all_three_providers_run_concurrently() {
        let openai_server = MockServer::start().await;
        let anthropic_server = MockServer::start().await;
        let gemini_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(openai_success_response()))
            .expect(1)
            .mount(&openai_server)
            .await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(anthropic_success_response()))
            .expect(1)
            .mount(&anthropic_server)
            .await;

        Mock::given(method("POST"))
            .and(path("/v1beta/models/gemini-pro:generateContent"))
            .respond_with(ResponseTemplate::new(200).set_body_json(gemini_success_response()))
            .expect(1)
            .mount(&gemini_server)
            .await;

        let client = Client::new();

        let (openai_resp, anthropic_resp, gemini_resp) = tokio::join!(
            client
                .post(format!("{}/v1/chat/completions", openai_server.uri()))
                .json(&openai_chat_request_body())
                .send(),
            client
                .post(format!("{}/v1/messages", anthropic_server.uri()))
                .json(&anthropic_messages_request_body())
                .send(),
            client
                .post(format!(
                    "{}/v1beta/models/gemini-pro:generateContent",
                    gemini_server.uri()
                ))
                .json(&gemini_generate_content_request_body())
                .send(),
        );

        let openai_body: Value = openai_resp.expect("openai ok").json().await.expect("json");
        let anthropic_body: Value = anthropic_resp
            .expect("anthropic ok")
            .json()
            .await
            .expect("json");
        let gemini_body: Value = gemini_resp.expect("gemini ok").json().await.expect("json");

        assert_eq!(openai_body["choices"][0]["message"]["content"], "pong");
        assert_eq!(anthropic_body["content"][0]["text"], "pong");
        assert_eq!(
            gemini_body["candidates"][0]["content"]["parts"][0]["text"],
            "pong"
        );
    }

    #[tokio::test]
    async fn openai_provider_integration_with_wiremock() {
        use ccuse_desktop_lib::providers::api::{ApiRequest, ChatMessage, HealthStatus, Provider};
        use ccuse_desktop_lib::providers::OpenAIProvider;

        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(openai_success_response()))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": []})))
            .mount(&server)
            .await;

        let provider =
            OpenAIProvider::new("oai-1", "MockOpenAI", server.uri(), "sk-test").expect("build");

        let request = ApiRequest {
            model: "gpt-5.4".into(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: "ping".into(),
                tool_call_id: None,
                tool_calls: vec![],
            }],
            temperature: Some(0.7),
            max_tokens: Some(64),
            stream: false,
            tools: vec![],
        };

        let response = provider.send_request(request).await.expect("ok");
        assert_eq!(response.id, "chatcmpl-mock");
        assert_eq!(response.choices[0].message.content, "pong");

        let health = provider.health_check().await.expect("health ok");
        assert_eq!(health, HealthStatus::Healthy);
    }
}

// ═══════════════════════════════════════════════════════════════════
// T1.0.5.09 — Fault injection tests
// ═══════════════════════════════════════════════════════════════════

mod fault_injection {
    use super::*;

    // ── 429 Rate Limiting ────────────────────────────────────────

    #[tokio::test]
    async fn openai_429_surfaces_rate_limit_with_retry_after() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(429)
                    .set_body_json(json!({
                        "error": {
                            "type": "rate_limit_exceeded",
                            "message": "Rate limit reached"
                        }
                    }))
                    .insert_header("Retry-After", "30"),
            )
            .mount(&server)
            .await;

        let client = Client::new();
        let resp = client
            .post(format!("{}/v1/chat/completions", server.uri()))
            .json(&openai_chat_request_body())
            .send()
            .await
            .expect("request should complete");

        assert_eq!(resp.status(), 429);
        let retry_after = resp.headers().get("Retry-After").expect("header present");
        assert_eq!(retry_after, "30");
        let body: Value = resp.json().await.expect("json");
        assert_eq!(body["error"]["type"], "rate_limit_exceeded");
    }

    #[tokio::test]
    async fn anthropic_429_surfaces_rate_limit_with_retry_after() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(
                ResponseTemplate::new(429)
                    .set_body_json(json!({
                        "type": "error",
                        "error": {
                            "type": "rate_limit_error",
                            "message": "Rate limit reached"
                        }
                    }))
                    .insert_header("Retry-After", "60"),
            )
            .mount(&server)
            .await;

        let client = Client::new();
        let resp = client
            .post(format!("{}/v1/messages", server.uri()))
            .json(&anthropic_messages_request_body())
            .send()
            .await
            .expect("request should complete");

        assert_eq!(resp.status(), 429);
        let retry_after = resp.headers().get("Retry-After").expect("header present");
        assert_eq!(retry_after, "60");
        let body: Value = resp.json().await.expect("json");
        assert_eq!(body["error"]["type"], "rate_limit_error");
    }

    #[tokio::test]
    async fn gemini_429_surfaces_rate_limit() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1beta/models/gemini-pro:generateContent"))
            .respond_with(
                ResponseTemplate::new(429)
                    .set_body_json(json!({
                        "error": {
                            "code": 429,
                            "message": "Resource has been exhausted",
                            "status": "RESOURCE_EXHAUSTED"
                        }
                    }))
                    .insert_header("Retry-After", "10"),
            )
            .mount(&server)
            .await;

        let client = Client::new();
        let resp = client
            .post(format!(
                "{}/v1beta/models/gemini-pro:generateContent",
                server.uri()
            ))
            .json(&gemini_generate_content_request_body())
            .send()
            .await
            .expect("request should complete");

        assert_eq!(resp.status(), 429);
        let body: Value = resp.json().await.expect("json");
        assert_eq!(body["error"]["status"], "RESOURCE_EXHAUSTED");
    }

    #[tokio::test]
    async fn openai_provider_maps_429_to_rate_limited_error() {
        use ccuse_desktop_lib::providers::api::{ApiRequest, ChatMessage, Provider, ProviderError};
        use ccuse_desktop_lib::providers::OpenAIProvider;

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(429)
                    .set_body_string("slow down")
                    .insert_header("Retry-After", "30"),
            )
            .mount(&server)
            .await;

        let provider =
            OpenAIProvider::new("oai-rl", "MockRL", server.uri(), "sk-test").expect("build");
        let request = ApiRequest {
            model: "gpt-5.4".into(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: "ping".into(),
                tool_call_id: None,
                tool_calls: vec![],
            }],
            temperature: None,
            max_tokens: None,
            stream: false,
            tools: vec![],
        };
        let err = provider.send_request(request).await.expect_err("must fail");
        assert!(matches!(err, ProviderError::RateLimited(_)));
        assert!(err.is_retriable());
    }

    // ── 5xx Server Errors ────────────────────────────────────────

    #[tokio::test]
    async fn openai_500_surfaces_server_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(500).set_body_json(json!({
                "error": {
                    "type": "server_error",
                    "message": "Internal server error"
                }
            })))
            .mount(&server)
            .await;

        let client = Client::new();
        let resp = client
            .post(format!("{}/v1/chat/completions", server.uri()))
            .json(&openai_chat_request_body())
            .send()
            .await
            .expect("request should complete");

        assert_eq!(resp.status(), 500);
        let body: Value = resp.json().await.expect("json");
        assert_eq!(body["error"]["type"], "server_error");
    }

    #[tokio::test]
    async fn anthropic_503_surfaces_overloaded_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(503).set_body_json(json!({
                "type": "error",
                "error": {
                    "type": "overloaded_error",
                    "message": "Overloaded"
                }
            })))
            .mount(&server)
            .await;

        let client = Client::new();
        let resp = client
            .post(format!("{}/v1/messages", server.uri()))
            .json(&anthropic_messages_request_body())
            .send()
            .await
            .expect("request should complete");

        assert_eq!(resp.status(), 503);
        let body: Value = resp.json().await.expect("json");
        assert_eq!(body["error"]["type"], "overloaded_error");
    }

    #[tokio::test]
    async fn gemini_500_surfaces_internal_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1beta/models/gemini-pro:generateContent"))
            .respond_with(ResponseTemplate::new(500).set_body_json(json!({
                "error": {
                    "code": 500,
                    "message": "Internal error",
                    "status": "INTERNAL"
                }
            })))
            .mount(&server)
            .await;

        let client = Client::new();
        let resp = client
            .post(format!(
                "{}/v1beta/models/gemini-pro:generateContent",
                server.uri()
            ))
            .json(&gemini_generate_content_request_body())
            .send()
            .await
            .expect("request should complete");

        assert_eq!(resp.status(), 500);
        let body: Value = resp.json().await.expect("json");
        assert_eq!(body["error"]["status"], "INTERNAL");
    }

    #[tokio::test]
    async fn openai_provider_maps_503_to_upstream_error() {
        use ccuse_desktop_lib::providers::api::{ApiRequest, ChatMessage, Provider, ProviderError};
        use ccuse_desktop_lib::providers::OpenAIProvider;

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(503).set_body_string("service unavailable"))
            .mount(&server)
            .await;

        let provider =
            OpenAIProvider::new("oai-5xx", "Mock5xx", server.uri(), "sk-test").expect("build");
        let request = ApiRequest {
            model: "gpt-5.4".into(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: "ping".into(),
                tool_call_id: None,
                tool_calls: vec![],
            }],
            temperature: None,
            max_tokens: None,
            stream: false,
            tools: vec![],
        };
        let err = provider.send_request(request).await.expect_err("must fail");
        match err {
            ProviderError::Upstream { status, body } => {
                assert_eq!(status, 503);
                assert!(body.contains("service unavailable"));
            }
            other => panic!("expected Upstream, got {other:?}"),
        }
    }

    // ── Timeout ──────────────────────────────────────────────────

    #[tokio::test]
    async fn openai_timeout_surfaces_as_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(openai_success_response())
                    .set_delay(Duration::from_secs(10)),
            )
            .mount(&server)
            .await;

        let client = short_timeout_client();
        let result = client
            .post(format!("{}/v1/chat/completions", server.uri()))
            .json(&openai_chat_request_body())
            .send()
            .await;

        let err = result.expect_err("should time out");
        assert!(err.is_timeout(), "error should be timeout: {err}");
    }

    #[tokio::test]
    async fn anthropic_timeout_surfaces_as_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(anthropic_success_response())
                    .set_delay(Duration::from_secs(10)),
            )
            .mount(&server)
            .await;

        let client = short_timeout_client();
        let result = client
            .post(format!("{}/v1/messages", server.uri()))
            .json(&anthropic_messages_request_body())
            .send()
            .await;

        let err = result.expect_err("should time out");
        assert!(err.is_timeout(), "error should be timeout: {err}");
    }

    #[tokio::test]
    async fn gemini_timeout_surfaces_as_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1beta/models/gemini-pro:generateContent"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(gemini_success_response())
                    .set_delay(Duration::from_secs(10)),
            )
            .mount(&server)
            .await;

        let client = short_timeout_client();
        let result = client
            .post(format!(
                "{}/v1beta/models/gemini-pro:generateContent",
                server.uri()
            ))
            .json(&gemini_generate_content_request_body())
            .send()
            .await;

        let err = result.expect_err("should time out");
        assert!(err.is_timeout(), "error should be timeout: {err}");
    }

    #[tokio::test]
    async fn openai_provider_timeout_maps_to_network_error() {
        use ccuse_desktop_lib::providers::api::{ApiRequest, ChatMessage, Provider, ProviderError};
        use ccuse_desktop_lib::providers::OpenAIProvider;

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(openai_success_response())
                    .set_delay(Duration::from_secs(60)),
            )
            .mount(&server)
            .await;

        let provider = OpenAIProvider::with_options_and_timeout(
            "oai-to",
            "MockTimeout",
            server.uri(),
            "sk-test",
            100,
            None,
            Duration::from_secs(2),
        )
        .expect("build");
        let request = ApiRequest {
            model: "gpt-5.4".into(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: "ping".into(),
                tool_call_id: None,
                tool_calls: vec![],
            }],
            temperature: None,
            max_tokens: None,
            stream: false,
            tools: vec![],
        };
        let err = provider.send_request(request).await.expect_err("must fail");
        assert!(
            matches!(err, ProviderError::Network(_)),
            "expected Network error, got {err:?}"
        );
        assert!(err.is_retriable());
    }

    // ── Connection Refused ───────────────────────────────────────

    #[tokio::test]
    async fn connection_refused_surfaces_as_reqwest_error() {
        let client = short_timeout_client();

        // Port 1 is almost certainly not listening and will be refused.
        let result = client
            .post("http://127.0.0.1:1/v1/chat/completions")
            .json(&openai_chat_request_body())
            .send()
            .await;

        let err = result.expect_err("should fail to connect");
        assert!(err.is_connect(), "error should be connection error: {err}");
    }

    #[tokio::test]
    async fn openai_provider_connection_refused_maps_to_network_error() {
        use ccuse_desktop_lib::providers::api::{ApiRequest, ChatMessage, Provider, ProviderError};
        use ccuse_desktop_lib::providers::OpenAIProvider;

        let provider = OpenAIProvider::new(
            "oai-refused",
            "MockRefused",
            "http://127.0.0.1:1",
            "sk-test",
        )
        .expect("build");

        let request = ApiRequest {
            model: "gpt-5.4".into(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: "ping".into(),
                tool_call_id: None,
                tool_calls: vec![],
            }],
            temperature: None,
            max_tokens: None,
            stream: false,
            tools: vec![],
        };

        let err = provider.send_request(request).await.expect_err("must fail");
        assert!(
            matches!(err, ProviderError::Network(_)),
            "expected Network error, got {err:?}"
        );
        assert!(err.is_retriable());
    }

    #[tokio::test]
    async fn health_check_connection_refused_maps_to_down_status() {
        use ccuse_desktop_lib::providers::api::{HealthStatus, Provider};
        use ccuse_desktop_lib::providers::OpenAIProvider;

        let provider = OpenAIProvider::new(
            "oai-refused-hc",
            "MockRefusedHC",
            "http://127.0.0.1:1",
            "sk-test",
        )
        .expect("build");

        let status = provider.health_check().await.expect("status");
        assert_eq!(status, HealthStatus::Down);
    }

    // ── Mixed fault scenarios ────────────────────────────────────

    #[tokio::test]
    async fn sequential_faults_across_all_three_providers() {
        let openai_server = MockServer::start().await;
        let anthropic_server = MockServer::start().await;
        let gemini_server = MockServer::start().await;

        // OpenAI returns 429
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(429)
                    .set_body_json(
                        json!({"error": {"type": "rate_limit_exceeded", "message": "slow down"}}),
                    )
                    .insert_header("Retry-After", "5"),
            )
            .mount(&openai_server)
            .await;

        // Anthropic returns 503
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(
                ResponseTemplate::new(503)
                    .set_body_json(json!({"type": "error", "error": {"type": "overloaded_error", "message": "overloaded"}})),
            )
            .mount(&anthropic_server)
            .await;

        // Gemini returns 500
        Mock::given(method("POST"))
            .and(path("/v1beta/models/gemini-pro:generateContent"))
            .respond_with(ResponseTemplate::new(500).set_body_json(
                json!({"error": {"code": 500, "message": "internal", "status": "INTERNAL"}}),
            ))
            .mount(&gemini_server)
            .await;

        let client = Client::new();

        let openai_resp = client
            .post(format!("{}/v1/chat/completions", openai_server.uri()))
            .json(&openai_chat_request_body())
            .send()
            .await
            .expect("ok");
        assert_eq!(openai_resp.status(), 429);

        let anthropic_resp = client
            .post(format!("{}/v1/messages", anthropic_server.uri()))
            .json(&anthropic_messages_request_body())
            .send()
            .await
            .expect("ok");
        assert_eq!(anthropic_resp.status(), 503);

        let gemini_resp = client
            .post(format!(
                "{}/v1beta/models/gemini-pro:generateContent",
                gemini_server.uri()
            ))
            .json(&gemini_generate_content_request_body())
            .send()
            .await
            .expect("ok");
        assert_eq!(gemini_resp.status(), 500);
    }
}
