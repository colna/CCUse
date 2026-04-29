//! T1.0.6.07 — end-to-end proxy tests for `/v1/chat/completions`.
//!
//! These tests run the real `ProxyServer`, inject a `SwitchEngine` with
//! wiremock-backed providers, and verify the HTTP route reaches upstream
//! providers through the normal dispatch path.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use ccuse_desktop_lib::converter::ModelMapping;
use ccuse_desktop_lib::providers::{
    HealthStatus, OpenAIProvider, ProviderKind, ProviderManager, ProviderWrapper,
};
use ccuse_desktop_lib::proxy::{ProxyAppState, ProxyServer, ServerError};
use ccuse_desktop_lib::switch::SwitchEngine;
use reqwest::StatusCode;
use serde_json::{json, Value};
use tokio::sync::{oneshot, RwLock};
use tokio::task::JoinHandle;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct ProviderSpec<'a> {
    id: &'a str,
    name: &'a str,
    priority: i32,
    server: &'a MockServer,
}

struct RunningProxy {
    base_url: String,
    shutdown_tx: oneshot::Sender<()>,
    handle: JoinHandle<Result<(), ServerError>>,
    manager: Arc<ProviderManager>,
}

impl RunningProxy {
    async fn shutdown(self) {
        let _ = self.shutdown_tx.send(());
        let result = tokio::time::timeout(Duration::from_secs(2), self.handle)
            .await
            .expect("proxy should shut down within 2s")
            .expect("proxy task should not panic");
        assert!(result.is_ok(), "proxy serve should return Ok");
    }
}

fn loopback_zero() -> SocketAddr {
    "127.0.0.1:0"
        .parse()
        .expect("loopback string is a valid SocketAddr")
}

async fn start_proxy_with_providers(specs: &[ProviderSpec<'_>]) -> RunningProxy {
    start_proxy_with_providers_and_mapping(specs, ModelMapping::new()).await
}

async fn start_proxy_with_providers_and_mapping(
    specs: &[ProviderSpec<'_>],
    model_mapping: ModelMapping,
) -> RunningProxy {
    start_proxy_with_providers_mapping_and_timeout(specs, model_mapping, None).await
}

async fn start_proxy_with_providers_mapping_and_timeout(
    specs: &[ProviderSpec<'_>],
    model_mapping: ModelMapping,
    non_streaming_timeout: Option<Duration>,
) -> RunningProxy {
    let manager = Arc::new(ProviderManager::new());
    for spec in specs {
        let provider = OpenAIProvider::with_options(
            spec.id,
            spec.name,
            spec.server.uri(),
            "sk-upstream-test",
            spec.priority,
            None,
        )
        .expect("build provider");
        let wrapper = Arc::new(ProviderWrapper::new(
            spec.id,
            spec.name,
            ProviderKind::Openai,
            spec.priority,
            None,
            true,
            Box::new(provider),
        ));
        manager.add(wrapper).await.expect("register provider");
    }

    let engine = Arc::new(SwitchEngine::new(Arc::clone(&manager)));
    let mapping = Arc::new(RwLock::new(model_mapping));
    let mut state = ProxyAppState::new(engine, mapping, Arc::clone(&manager));
    if let Some(timeout) = non_streaming_timeout {
        state = state.with_non_streaming_timeout(timeout);
    }
    let server = ProxyServer::bind(loopback_zero())
        .await
        .expect("bind proxy");
    let base_url = format!("http://{}", server.local_addr());
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let handle = tokio::spawn(server.serve_with_shutdown(state, async move {
        let _ = shutdown_rx.await;
    }));
    tokio::time::sleep(Duration::from_millis(50)).await;

    RunningProxy {
        base_url,
        shutdown_tx,
        handle,
        manager,
    }
}

fn chat_request(stream: bool) -> Value {
    json!({
        "model": "gpt-4o",
        "messages": [{"role": "user", "content": "ping"}],
        "temperature": 0.7,
        "max_tokens": 64,
        "stream": stream
    })
}

fn chat_request_with_model(model: &str) -> Value {
    json!({
        "model": model,
        "messages": [{"role": "user", "content": "ping"}],
        "stream": false
    })
}

fn chat_request_with_tools() -> Value {
    json!({
        "model": "gpt-4o",
        "messages": [{"role": "user", "content": "weather in Tokyo?"}],
        "tools": [{
            "type": "function",
            "function": {
                "name": "get_weather",
                "description": "Get weather",
                "parameters": {
                    "type": "object",
                    "properties": {"city": {"type": "string"}}
                }
            }
        }],
        "stream": false
    })
}

fn anthropic_messages_request() -> Value {
    json!({
        "model": "claude-3-5-sonnet-20241022",
        "max_tokens": 128,
        "system": "You are terse.",
        "messages": [{"role": "user", "content": "ping"}],
        "stream": false
    })
}

fn anthropic_messages_request_with_model(model: &str) -> Value {
    json!({
        "model": model,
        "max_tokens": 128,
        "messages": [{"role": "user", "content": "ping"}],
        "stream": false
    })
}

fn anthropic_messages_stream_request() -> Value {
    json!({
        "model": "claude-3-5-sonnet-20241022",
        "max_tokens": 128,
        "system": "You are terse.",
        "messages": [{"role": "user", "content": "ping"}],
        "stream": true
    })
}

fn anthropic_messages_tool_request() -> Value {
    json!({
        "model": "claude-3-5-sonnet-20241022",
        "max_tokens": 128,
        "messages": [
            {"role": "user", "content": "weather in Tokyo?"},
            {"role": "assistant", "content": [{
                "type": "tool_use",
                "id": "toolu_weather",
                "name": "get_weather",
                "input": {"city": "Tokyo"}
            }]},
            {"role": "user", "content": [{
                "type": "tool_result",
                "tool_use_id": "toolu_weather",
                "content": "sunny 25C"
            }]}
        ],
        "tools": [{
            "name": "get_weather",
            "description": "Get weather",
            "input_schema": {
                "type": "object",
                "properties": {"city": {"type": "string"}},
                "required": ["city"]
            }
        }],
        "stream": false
    })
}

fn openai_text_response(content: &str) -> Value {
    json!({
        "id": "chatcmpl-proxy-e2e",
        "object": "chat.completion",
        "created": 1_700_000_000_u64,
        "model": "gpt-4o",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": content},
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 4, "completion_tokens": 2, "total_tokens": 6}
    })
}

fn models_response(ids: &[&str]) -> Value {
    json!({
        "object": "list",
        "data": ids
            .iter()
            .map(|id| json!({"id": id, "object": "model"}))
            .collect::<Vec<_>>(),
    })
}

fn openai_text_response_with_finish_reason(content: &str, finish_reason: &str) -> Value {
    json!({
        "id": "chatcmpl-proxy-e2e",
        "object": "chat.completion",
        "created": 1_700_000_000_u64,
        "model": "gpt-4o",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": content},
            "finish_reason": finish_reason
        }],
        "usage": {"prompt_tokens": 4, "completion_tokens": 2, "total_tokens": 6}
    })
}

fn openai_tool_call_response() -> Value {
    json!({
        "id": "chatcmpl-tool",
        "object": "chat.completion",
        "created": 1_700_000_000_u64,
        "model": "gpt-4o",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_weather",
                    "type": "function",
                    "function": {
                        "name": "get_weather",
                        "arguments": "{\"city\":\"Tokyo\"}"
                    }
                }]
            },
            "finish_reason": "tool_calls"
        }],
        "usage": {"prompt_tokens": 12, "completion_tokens": 4, "total_tokens": 16}
    })
}

fn assert_contains_in_order(body: &str, markers: &[&str]) {
    let mut offset = 0;
    for marker in markers {
        let found = body[offset..]
            .find(marker)
            .unwrap_or_else(|| panic!("expected marker after byte {offset}: {marker}"));
        offset += found + marker.len();
    }
}

#[tokio::test]
async fn models_aggregates_providers_with_namespaced_deduped_ids() {
    let primary = MockServer::start().await;
    let backup = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(models_response(&["gpt-4o", "gpt-4o"])),
        )
        .expect(1)
        .mount(&primary)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(models_response(&["gpt-4o"])))
        .expect(1)
        .mount(&backup)
        .await;
    let proxy = start_proxy_with_providers(&[
        ProviderSpec {
            id: "models-primary",
            name: "Models Primary",
            priority: 1,
            server: &primary,
        },
        ProviderSpec {
            id: "models-backup",
            name: "Models Backup",
            priority: 2,
            server: &backup,
        },
    ])
    .await;

    let response = reqwest::get(format!("{}/v1/models", proxy.base_url))
        .await
        .expect("models request");

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.expect("response json");
    let ids = body["data"]
        .as_array()
        .expect("data array")
        .iter()
        .map(|model| model["id"].as_str().expect("id"))
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["models-primary::gpt-4o", "models-backup::gpt-4o"],);
    assert_eq!(body["data"][0]["owned_by"], "models-primary");
    assert_eq!(body["data"][1]["owned_by"], "models-backup");

    proxy.shutdown().await;
}

#[tokio::test]
async fn models_returns_partial_results_when_one_provider_fails() {
    let failed = MockServer::start().await;
    let healthy = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(500).set_body_string("models unavailable"))
        .expect(1)
        .mount(&failed)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(models_response(&["gpt-4o-mini"])))
        .expect(1)
        .mount(&healthy)
        .await;
    let proxy = start_proxy_with_providers(&[
        ProviderSpec {
            id: "models-failed",
            name: "Models Failed",
            priority: 1,
            server: &failed,
        },
        ProviderSpec {
            id: "models-healthy",
            name: "Models Healthy",
            priority: 2,
            server: &healthy,
        },
    ])
    .await;

    let response = reqwest::get(format!("{}/v1/models", proxy.base_url))
        .await
        .expect("models request");

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.expect("response json");
    let data = body["data"].as_array().expect("data array");
    assert_eq!(data.len(), 1);
    assert_eq!(data[0]["id"], "models-healthy::gpt-4o-mini");

    proxy.shutdown().await;
}

#[tokio::test]
async fn models_returns_empty_list_when_upstream_has_no_models() {
    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(models_response(&[])))
        .expect(1)
        .mount(&upstream)
        .await;
    let proxy = start_proxy_with_providers(&[ProviderSpec {
        id: "models-empty",
        name: "Models Empty",
        priority: 1,
        server: &upstream,
    }])
    .await;

    let response = reqwest::get(format!("{}/v1/models", proxy.base_url))
        .await
        .expect("models request");

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.expect("response json");
    assert_eq!(body["object"], "list");
    assert!(
        body["data"].as_array().is_some_and(Vec::is_empty),
        "empty upstream model list should stay an empty data array",
    );

    proxy.shutdown().await;
}

#[tokio::test]
async fn chat_completions_applies_exact_provider_model_mapping() {
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(openai_text_response("mapped")))
        .expect(1)
        .mount(&upstream)
        .await;

    let mut mapping = ModelMapping::new();
    mapping.set_mapping(
        "client-fast",
        "mapping-exact-provider",
        "provider-exact-model",
    );
    let proxy = start_proxy_with_providers_and_mapping(
        &[ProviderSpec {
            id: "mapping-exact-provider",
            name: "Mapping Exact",
            priority: 1,
            server: &upstream,
        }],
        mapping,
    )
    .await;

    let response = reqwest::Client::new()
        .post(format!("{}/v1/chat/completions", proxy.base_url))
        .json(&chat_request_with_model("client-fast"))
        .send()
        .await
        .expect("proxy request");

    assert_eq!(response.status(), StatusCode::OK);
    let received = upstream.received_requests().await.expect("received");
    let upstream_body: Value = serde_json::from_slice(&received[0].body).expect("json");
    assert_eq!(upstream_body["model"], "provider-exact-model");

    proxy.shutdown().await;
}

#[tokio::test]
async fn anthropic_messages_applies_wildcard_model_mapping() {
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(openai_text_response("anthropic mapped")),
        )
        .expect(1)
        .mount(&upstream)
        .await;

    let mut mapping = ModelMapping::new();
    mapping.set_mapping("client-slow", "*", "wildcard-upstream-model");
    let proxy = start_proxy_with_providers_and_mapping(
        &[ProviderSpec {
            id: "mapping-wildcard-provider",
            name: "Mapping Wildcard",
            priority: 1,
            server: &upstream,
        }],
        mapping,
    )
    .await;

    let response = reqwest::Client::new()
        .post(format!("{}/v1/messages", proxy.base_url))
        .json(&anthropic_messages_request_with_model("client-slow"))
        .send()
        .await
        .expect("proxy request");

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.expect("response json");
    assert_eq!(body["content"][0]["text"], "anthropic mapped");
    let received = upstream.received_requests().await.expect("received");
    let upstream_body: Value = serde_json::from_slice(&received[0].body).expect("json");
    assert_eq!(upstream_body["model"], "wildcard-upstream-model");

    proxy.shutdown().await;
}

#[tokio::test]
async fn chat_completions_keeps_original_model_without_mapping() {
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(openai_text_response("unmapped")))
        .expect(1)
        .mount(&upstream)
        .await;
    let proxy = start_proxy_with_providers(&[ProviderSpec {
        id: "mapping-unmapped-provider",
        name: "Mapping Unmapped",
        priority: 1,
        server: &upstream,
    }])
    .await;

    let response = reqwest::Client::new()
        .post(format!("{}/v1/chat/completions", proxy.base_url))
        .json(&chat_request_with_model("client-unmapped"))
        .send()
        .await
        .expect("proxy request");

    assert_eq!(response.status(), StatusCode::OK);
    let received = upstream.received_requests().await.expect("received");
    let upstream_body: Value = serde_json::from_slice(&received[0].body).expect("json");
    assert_eq!(upstream_body["model"], "client-unmapped");

    proxy.shutdown().await;
}

#[tokio::test]
async fn chat_completions_non_streaming_handler_times_out() {
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(250))
                .set_body_json(openai_text_response("too late")),
        )
        .mount(&upstream)
        .await;
    let proxy = start_proxy_with_providers_mapping_and_timeout(
        &[ProviderSpec {
            id: "timeout-provider",
            name: "Timeout Provider",
            priority: 1,
            server: &upstream,
        }],
        ModelMapping::new(),
        Some(Duration::from_millis(25)),
    )
    .await;

    let response = reqwest::Client::new()
        .post(format!("{}/v1/chat/completions", proxy.base_url))
        .json(&chat_request(false))
        .send()
        .await
        .expect("proxy request");

    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    let body: Value = response.json().await.expect("response json");
    assert_eq!(body["error"]["type"], "request_timeout");

    proxy.shutdown().await;
}

#[tokio::test]
async fn chat_completions_streaming_ignores_non_streaming_handler_timeout() {
    let upstream = MockServer::start().await;
    let sse = "data: {\"id\":\"chatcmpl-timeout-stream\",\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"ok\"},\"finish_reason\":null}]}\n\n\
               data: [DONE]\n\n";
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(75))
                .set_body_string(sse)
                .insert_header("content-type", "text/event-stream"),
        )
        .expect(1)
        .mount(&upstream)
        .await;
    let proxy = start_proxy_with_providers_mapping_and_timeout(
        &[ProviderSpec {
            id: "stream-timeout-provider",
            name: "Stream Timeout Provider",
            priority: 1,
            server: &upstream,
        }],
        ModelMapping::new(),
        Some(Duration::from_millis(10)),
    )
    .await;

    let response = reqwest::Client::new()
        .post(format!("{}/v1/chat/completions", proxy.base_url))
        .json(&chat_request(true))
        .send()
        .await
        .expect("proxy request");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.text().await.expect("sse text");
    assert!(body.contains("data: [DONE]"));

    proxy.shutdown().await;
}

#[tokio::test]
async fn anthropic_messages_dispatches_non_streaming_request_to_upstream() {
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(openai_text_response("pong")))
        .expect(1)
        .mount(&upstream)
        .await;
    let proxy = start_proxy_with_providers(&[ProviderSpec {
        id: "anthropic-inbound",
        name: "Anthropic Inbound",
        priority: 1,
        server: &upstream,
    }])
    .await;

    let response = reqwest::Client::new()
        .post(format!("{}/v1/messages", proxy.base_url))
        .json(&anthropic_messages_request())
        .send()
        .await
        .expect("proxy request");

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.expect("response json");
    assert_eq!(body["type"], "message");
    assert_eq!(body["role"], "assistant");
    assert_eq!(body["content"][0]["type"], "text");
    assert_eq!(body["content"][0]["text"], "pong");
    assert_eq!(body["usage"]["input_tokens"], 4);
    assert_eq!(body["usage"]["output_tokens"], 2);

    let received = upstream.received_requests().await.expect("received");
    let upstream_body: Value = serde_json::from_slice(&received[0].body).expect("json");
    assert_eq!(upstream_body["model"], "claude-3-5-sonnet-20241022");
    assert_eq!(upstream_body["messages"][0]["role"], "system");
    assert_eq!(upstream_body["messages"][0]["content"], "You are terse.");
    assert_eq!(upstream_body["messages"][1]["role"], "user");
    assert_eq!(upstream_body["messages"][1]["content"], "ping");
    assert_eq!(upstream_body["max_tokens"], 128);
    assert_eq!(upstream_body["stream"], false);

    proxy.shutdown().await;
}

#[tokio::test]
async fn anthropic_messages_preserves_tool_use_round_trip() {
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(openai_tool_call_response()))
        .expect(1)
        .mount(&upstream)
        .await;
    let proxy = start_proxy_with_providers(&[ProviderSpec {
        id: "anthropic-tools",
        name: "Anthropic Tools",
        priority: 1,
        server: &upstream,
    }])
    .await;

    let response = reqwest::Client::new()
        .post(format!("{}/v1/messages", proxy.base_url))
        .json(&anthropic_messages_tool_request())
        .send()
        .await
        .expect("proxy request");

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.expect("response json");
    assert_eq!(body["content"][0]["type"], "tool_use");
    assert_eq!(body["content"][0]["id"], "call_weather");
    assert_eq!(body["content"][0]["name"], "get_weather");
    assert_eq!(body["content"][0]["input"]["city"], "Tokyo");
    assert_eq!(body["stop_reason"], "tool_use");
    assert_eq!(body["usage"]["input_tokens"], 12);
    assert_eq!(body["usage"]["output_tokens"], 4);

    let received = upstream.received_requests().await.expect("received");
    let upstream_body: Value = serde_json::from_slice(&received[0].body).expect("json");
    assert_eq!(upstream_body["tools"][0]["function"]["name"], "get_weather");
    assert_eq!(
        upstream_body["tools"][0]["function"]["parameters"]["required"][0],
        "city",
    );
    assert_eq!(upstream_body["messages"][1]["role"], "assistant");
    assert_eq!(
        upstream_body["messages"][1]["tool_calls"][0]["function"]["name"],
        "get_weather",
    );
    let arguments: Value = serde_json::from_str(
        upstream_body["messages"][1]["tool_calls"][0]["function"]["arguments"]
            .as_str()
            .expect("arguments string"),
    )
    .expect("arguments json");
    assert_eq!(arguments["city"], "Tokyo");
    assert_eq!(upstream_body["messages"][2]["role"], "tool");
    assert_eq!(
        upstream_body["messages"][2]["tool_call_id"],
        "toolu_weather"
    );
    assert_eq!(upstream_body["messages"][2]["content"], "sunny 25C");

    proxy.shutdown().await;
}

#[tokio::test]
async fn anthropic_messages_maps_openai_length_to_max_tokens_stop_reason() {
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(openai_text_response_with_finish_reason("partial", "length")),
        )
        .expect(1)
        .mount(&upstream)
        .await;
    let proxy = start_proxy_with_providers(&[ProviderSpec {
        id: "anthropic-stop-reason",
        name: "Anthropic Stop Reason",
        priority: 1,
        server: &upstream,
    }])
    .await;

    let response = reqwest::Client::new()
        .post(format!("{}/v1/messages", proxy.base_url))
        .json(&anthropic_messages_request())
        .send()
        .await
        .expect("proxy request");

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.expect("response json");
    assert_eq!(body["content"][0]["text"], "partial");
    assert_eq!(body["stop_reason"], "max_tokens");

    proxy.shutdown().await;
}

#[tokio::test]
async fn anthropic_messages_streams_anthropic_sse_events() {
    let upstream = MockServer::start().await;
    let sse = "data: {\"id\":\"chatcmpl-anthropic-stream\",\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\"},\"finish_reason\":null}]}\n\n\
               data: {\"id\":\"chatcmpl-anthropic-stream\",\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hel\"},\"finish_reason\":null}]}\n\n\
               data: {\"id\":\"chatcmpl-anthropic-stream\",\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"lo\"},\"finish_reason\":null}]}\n\n\
               data: {\"id\":\"chatcmpl-anthropic-stream\",\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n\
               data: [DONE]\n\n";
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(sse)
                .insert_header("content-type", "text/event-stream"),
        )
        .expect(1)
        .mount(&upstream)
        .await;
    let proxy = start_proxy_with_providers(&[ProviderSpec {
        id: "anthropic-stream",
        name: "Anthropic Stream",
        priority: 1,
        server: &upstream,
    }])
    .await;

    let response = reqwest::Client::new()
        .post(format!("{}/v1/messages", proxy.base_url))
        .json(&anthropic_messages_stream_request())
        .send()
        .await
        .expect("proxy request");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.text().await.expect("sse text");
    assert!(body.contains("event: message_start"));
    assert!(body.contains("event: content_block_start"));
    assert!(body.contains("event: content_block_delta"));
    assert!(body.contains("event: content_block_stop"));
    assert!(body.contains("event: message_delta"));
    assert!(body.contains("event: message_stop"));
    assert!(body.contains("\"text\":\"Hel\""));
    assert!(body.contains("\"text\":\"lo\""));
    assert!(body.contains("\"stop_reason\":\"end_turn\""));
    assert!(!body.contains("data: [DONE]"));
    assert_contains_in_order(
        &body,
        &[
            "event: message_start",
            "event: content_block_start",
            "\"text\":\"Hel\"",
            "\"text\":\"lo\"",
            "event: content_block_stop",
            "event: message_delta",
            "event: message_stop",
        ],
    );

    let received = upstream.received_requests().await.expect("received");
    let upstream_body: Value = serde_json::from_slice(&received[0].body).expect("json");
    assert_eq!(upstream_body["stream"], true);

    proxy.shutdown().await;
}

#[tokio::test]
async fn anthropic_messages_streams_tool_use_events_in_order() {
    let upstream = MockServer::start().await;
    let sse = r#"data: {"id":"chatcmpl-tool-stream","model":"gpt-4o","choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}

data: {"id":"chatcmpl-tool-stream","model":"gpt-4o","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_weather","type":"function","function":{"name":"get_weather","arguments":"{\"city\":"}}]},"finish_reason":null}]}

data: {"id":"chatcmpl-tool-stream","model":"gpt-4o","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"Tokyo\"}"}}]},"finish_reason":null}]}

data: {"id":"chatcmpl-tool-stream","model":"gpt-4o","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}

data: [DONE]

"#;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(sse)
                .insert_header("content-type", "text/event-stream"),
        )
        .expect(1)
        .mount(&upstream)
        .await;
    let proxy = start_proxy_with_providers(&[ProviderSpec {
        id: "anthropic-tool-stream",
        name: "Anthropic Tool Stream",
        priority: 1,
        server: &upstream,
    }])
    .await;

    let response = reqwest::Client::new()
        .post(format!("{}/v1/messages", proxy.base_url))
        .json(&anthropic_messages_stream_request())
        .send()
        .await
        .expect("proxy request");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.text().await.expect("sse text");
    assert_contains_in_order(
        &body,
        &[
            "event: message_start",
            "event: content_block_start",
            "\"name\":\"get_weather\"",
            "\"type\":\"tool_use\"",
            "\"type\":\"input_json_delta\"",
            "event: content_block_stop",
            "event: message_delta",
            "\"stop_reason\":\"tool_use\"",
            "event: message_stop",
        ],
    );
    assert!(!body.contains("data: [DONE]"));

    let received = upstream.received_requests().await.expect("received");
    let upstream_body: Value = serde_json::from_slice(&received[0].body).expect("json");
    assert_eq!(upstream_body["stream"], true);

    proxy.shutdown().await;
}

#[tokio::test]
async fn chat_completions_dispatches_text_request_to_upstream() {
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(openai_text_response("pong")))
        .expect(1)
        .mount(&upstream)
        .await;
    let proxy = start_proxy_with_providers(&[ProviderSpec {
        id: "openai-primary",
        name: "OpenAI Primary",
        priority: 1,
        server: &upstream,
    }])
    .await;

    let response = reqwest::Client::new()
        .post(format!("{}/v1/chat/completions", proxy.base_url))
        .json(&chat_request(false))
        .send()
        .await
        .expect("proxy request");

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.expect("response json");
    assert_eq!(body["choices"][0]["message"]["content"], "pong");
    assert_eq!(body["usage"]["total_tokens"], 6);

    let received = upstream.received_requests().await.expect("received");
    let upstream_body: Value = serde_json::from_slice(&received[0].body).expect("json");
    assert_eq!(upstream_body["model"], "gpt-4o");
    assert_eq!(upstream_body["messages"][0]["content"], "ping");
    assert_eq!(upstream_body["stream"], false);

    proxy.shutdown().await;
}

#[tokio::test]
async fn chat_completions_forwards_tool_definitions_to_upstream() {
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(openai_text_response("tool-ready")))
        .expect(1)
        .mount(&upstream)
        .await;
    let proxy = start_proxy_with_providers(&[ProviderSpec {
        id: "openai-tools",
        name: "OpenAI Tools",
        priority: 1,
        server: &upstream,
    }])
    .await;

    let response = reqwest::Client::new()
        .post(format!("{}/v1/chat/completions", proxy.base_url))
        .json(&chat_request_with_tools())
        .send()
        .await
        .expect("proxy request");

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.expect("response json");
    assert_eq!(body["choices"][0]["message"]["content"], "tool-ready");

    let received = upstream.received_requests().await.expect("received");
    let upstream_body: Value = serde_json::from_slice(&received[0].body).expect("json");
    assert_eq!(upstream_body["tools"][0]["function"]["name"], "get_weather");
    assert_eq!(
        upstream_body["tools"][0]["function"]["parameters"]["properties"]["city"]["type"],
        "string",
    );

    proxy.shutdown().await;
}

#[tokio::test]
async fn chat_completions_streams_sse_response_to_client() {
    let upstream = MockServer::start().await;
    let sse = "data: {\"id\":\"chatcmpl-stream\",\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"Hel\"},\"finish_reason\":null}]}\n\n\
               data: {\"id\":\"chatcmpl-stream\",\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"lo\"},\"finish_reason\":null}]}\n\n\
               data: [DONE]\n\n";
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(sse)
                .insert_header("content-type", "text/event-stream"),
        )
        .expect(1)
        .mount(&upstream)
        .await;
    let proxy = start_proxy_with_providers(&[ProviderSpec {
        id: "openai-stream",
        name: "OpenAI Stream",
        priority: 1,
        server: &upstream,
    }])
    .await;

    let response = reqwest::Client::new()
        .post(format!("{}/v1/chat/completions", proxy.base_url))
        .json(&chat_request(true))
        .send()
        .await
        .expect("proxy request");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("text/event-stream"),
    );
    let body = response.text().await.expect("sse text");
    assert!(body.contains("\"Hel\""));
    assert!(body.contains("\"lo\""));
    assert!(body.contains("data: [DONE]"));

    let received = upstream.received_requests().await.expect("received");
    let upstream_body: Value = serde_json::from_slice(&received[0].body).expect("json");
    assert_eq!(upstream_body["stream"], true);

    proxy.shutdown().await;
}

#[tokio::test]
async fn chat_completions_retries_after_429_and_uses_next_provider() {
    let primary = MockServer::start().await;
    let backup = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(429)
                .set_body_json(json!({"error": {"type": "rate_limit_exceeded"}})),
        )
        .expect(1)
        .mount(&primary)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(openai_text_response("backup")))
        .expect(1)
        .mount(&backup)
        .await;
    let proxy = start_proxy_with_providers(&[
        ProviderSpec {
            id: "primary",
            name: "Primary",
            priority: 1,
            server: &primary,
        },
        ProviderSpec {
            id: "backup",
            name: "Backup",
            priority: 2,
            server: &backup,
        },
    ])
    .await;

    let response = reqwest::Client::new()
        .post(format!("{}/v1/chat/completions", proxy.base_url))
        .json(&chat_request(false))
        .send()
        .await
        .expect("proxy request");

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.expect("response json");
    assert_eq!(body["choices"][0]["message"]["content"], "backup");

    let first = proxy
        .manager
        .get("primary")
        .await
        .expect("primary provider");
    assert_eq!(first.state.health().await, HealthStatus::Degraded);

    proxy.shutdown().await;
}
