//! T1.0.6.07 — end-to-end proxy tests for `/v1/chat/completions`.
//!
//! These tests run the real `ProxyServer`, inject a `SwitchEngine` with
//! wiremock-backed providers, and verify the HTTP route reaches upstream
//! providers through the normal dispatch path.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use ccuse_desktop_lib::db::{open_database, run_migrations, Database};
use ccuse_desktop_lib::providers::{
    AnthropicProvider, HealthStatus, OpenAIProvider, ProviderKind, ProviderManager, ProviderWrapper,
};
use ccuse_desktop_lib::proxy::{ProxyAppState, ProxyServer, ServerError};
use ccuse_desktop_lib::switch::history::SwitchHistoryRepository;
use ccuse_desktop_lib::switch::request_log::RequestLogRepository;
use ccuse_desktop_lib::switch::SwitchEngine;
use reqwest::StatusCode;
use serde_json::{json, Value};
use tempfile::TempDir;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use wiremock::matchers::{body_partial_json, method, path};
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
    start_proxy_with_providers_and_timeout(specs, None).await
}

async fn start_proxy_with_providers_and_timeout(
    specs: &[ProviderSpec<'_>],
    non_streaming_timeout: Option<Duration>,
) -> RunningProxy {
    start_proxy_with_providers_timeout_and_request_log(specs, non_streaming_timeout, None).await
}

async fn start_proxy_with_providers_timeout_and_request_log(
    specs: &[ProviderSpec<'_>],
    non_streaming_timeout: Option<Duration>,
    request_log_db: Option<Database>,
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
        .expect("build openai-compatible provider");
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
    let mut state = ProxyAppState::new(engine, Arc::clone(&manager));
    if let Some(db) = request_log_db {
        state = state.with_monitoring(db);
    }
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

async fn start_proxy_with_native_anthropic_provider(spec: &ProviderSpec<'_>) -> RunningProxy {
    let manager = Arc::new(ProviderManager::new());
    let provider = AnthropicProvider::with_options(
        spec.id,
        spec.name,
        spec.server.uri(),
        "sk-upstream-test",
        spec.priority,
        None,
    )
    .expect("build native anthropic provider");
    let wrapper = Arc::new(ProviderWrapper::new(
        spec.id,
        spec.name,
        ProviderKind::Anthropic,
        spec.priority,
        None,
        true,
        Box::new(provider),
    ));
    manager.add(wrapper).await.expect("register provider");

    let engine = Arc::new(SwitchEngine::new(Arc::clone(&manager)));
    let state = ProxyAppState::new(engine, Arc::clone(&manager));
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
        "model": "gpt-5.5-instant",
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

fn chat_request_without_model() -> Value {
    json!({
        "messages": [{"role": "user", "content": "ping"}],
        "stream": false
    })
}

fn chat_request_with_image() -> Value {
    json!({
        "model": "gpt-5.5-instant",
        "messages": [{
            "role": "user",
            "content": [
                {"type": "text", "text": "describe this"},
                {
                    "type": "image_url",
                    "image_url": {
                        "url": "data:image/png;base64,abc123",
                        "detail": "high"
                    }
                }
            ]
        }],
        "stream": false
    })
}

fn chat_stream_request_with_image() -> Value {
    let mut body = chat_request_with_image();
    body["stream"] = json!(true);
    body
}

fn chat_request_with_tools() -> Value {
    json!({
        "model": "gpt-5.5-instant",
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
        "model": "claude-sonnet-4-6",
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

fn anthropic_messages_request_without_model() -> Value {
    json!({
        "max_tokens": 128,
        "messages": [{"role": "user", "content": "ping"}],
        "stream": false
    })
}

fn anthropic_messages_request_with_base64_image() -> Value {
    json!({
        "model": "claude-sonnet-4-6",
        "max_tokens": 128,
        "messages": [{
            "role": "user",
            "content": [
                {"type": "text", "text": "describe this"},
                {
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": "image/png",
                        "data": "abc123"
                    }
                }
            ]
        }],
        "stream": false
    })
}

fn anthropic_messages_stream_request_with_base64_image() -> Value {
    let mut body = anthropic_messages_request_with_base64_image();
    body["stream"] = json!(true);
    body
}

fn anthropic_messages_stream_request() -> Value {
    json!({
        "model": "claude-sonnet-4-6",
        "max_tokens": 128,
        "system": "You are terse.",
        "messages": [{"role": "user", "content": "ping"}],
        "stream": true
    })
}

fn anthropic_messages_tool_request() -> Value {
    json!({
        "model": "claude-sonnet-4-6",
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

fn anthropic_messages_parallel_tool_request() -> Value {
    json!({
        "model": "claude-sonnet-4-6",
        "max_tokens": 128,
        "messages": [
            {"role": "user", "content": "run both tools"},
            {"role": "assistant", "content": [
                {
                    "type": "tool_use",
                    "id": "toolu_read",
                    "name": "Read",
                    "input": {"file_path": "README.md"}
                },
                {
                    "type": "tool_use",
                    "id": "toolu_bash",
                    "name": "Bash",
                    "input": {"command": "pwd"}
                }
            ]},
            {"role": "user", "content": [
                {
                    "type": "tool_result",
                    "tool_use_id": "toolu_read",
                    "content": "file contents"
                },
                {
                    "type": "tool_result",
                    "tool_use_id": "toolu_bash",
                    "content": "/tmp/project"
                }
            ]}
        ],
        "tools": [
            {
                "name": "Read",
                "input_schema": {"type": "object"}
            },
            {
                "name": "Bash",
                "input_schema": {"type": "object"}
            }
        ],
        "stream": false
    })
}

fn openai_text_response(content: &str) -> Value {
    json!({
        "id": "chatcmpl-proxy-e2e",
        "object": "chat.completion",
        "created": 1_700_000_000_u64,
        "model": "gpt-5.5-instant",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": content},
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 4, "completion_tokens": 2, "total_tokens": 6}
    })
}

fn openai_text_response_without_usage(content: &str) -> Value {
    let mut body = openai_text_response(content);
    body.as_object_mut()
        .expect("openai text response is an object")
        .remove("usage");
    body
}

fn request_log_database_with_provider(provider_id: &str) -> (TempDir, Database) {
    monitoring_database_with_providers(&[provider_id])
}

fn monitoring_database_with_providers(provider_ids: &[&str]) -> (TempDir, Database) {
    let dir = TempDir::new().expect("tempdir");
    let db = open_database(dir.path().join("monitoring.db")).expect("open database");
    run_migrations(&db).expect("run migrations");
    db.with_connection(|conn| {
        for provider_id in provider_ids {
            conn.execute(
                "INSERT INTO providers (id, name, kind, base_url, encrypted_api_key, enabled) \
                 VALUES (?1, ?1, 'openai', 'https://api.example.test', x'00', 1)",
                rusqlite::params![provider_id],
            )?;
        }
        Ok(())
    })
    .expect("seed provider rows");
    (dir, db)
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
        "model": "gpt-5.5-instant",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": content},
            "finish_reason": finish_reason
        }],
        "usage": {"prompt_tokens": 4, "completion_tokens": 2, "total_tokens": 6}
    })
}

fn anthropic_text_response(content: &str) -> Value {
    json!({
        "id": "msg_native_proxy_e2e",
        "type": "message",
        "role": "assistant",
        "model": "claude-sonnet-4-20250514",
        "content": [{"type": "text", "text": content}],
        "stop_reason": "end_turn",
        "stop_sequence": null,
        "usage": {"input_tokens": 4, "output_tokens": 2}
    })
}

fn native_anthropic_sse() -> &'static str {
    r#"event: message_start
data: {"type":"message_start","message":{"id":"msg_native_stream","type":"message","role":"assistant","model":"claude-sonnet-4-20250514","content":[],"stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":1,"output_tokens":0}}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"hi"}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":1}}

event: message_stop
data: {"type":"message_stop"}

"#
}

fn openai_tool_call_response() -> Value {
    json!({
        "id": "chatcmpl-tool",
        "object": "chat.completion",
        "created": 1_700_000_000_u64,
        "model": "gpt-5.5-instant",
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
            ResponseTemplate::new(200)
                .set_body_json(models_response(&["gpt-5.5-instant", "gpt-5.5-instant"])),
        )
        .expect(1)
        .mount(&primary)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(models_response(&["gpt-5.5-instant"])),
        )
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
    assert_eq!(
        ids,
        vec![
            "models-primary::gpt-5.5-instant",
            "models-backup::gpt-5.5-instant"
        ],
    );
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
        .respond_with(
            ResponseTemplate::new(200).set_body_json(models_response(&["gpt-5.5-instant"])),
        )
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
    assert_eq!(data[0]["id"], "models-healthy::gpt-5.5-instant");

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
async fn chat_completions_uses_openai_default_model_when_client_model_is_present() {
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(openai_text_response("default")))
        .expect(1)
        .mount(&upstream)
        .await;

    let proxy = start_proxy_with_providers(&[ProviderSpec {
        id: "default-model-openai",
        name: "Default Model OpenAI",
        priority: 1,
        server: &upstream,
    }])
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
    assert_eq!(upstream_body["model"], "gpt-5.5");

    proxy.shutdown().await;
}

#[tokio::test]
async fn chat_completions_without_model_uses_openai_default_model() {
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(openai_text_response("default")))
        .expect(1)
        .mount(&upstream)
        .await;

    let proxy = start_proxy_with_providers(&[ProviderSpec {
        id: "missing-model-openai",
        name: "Missing Model OpenAI",
        priority: 1,
        server: &upstream,
    }])
    .await;

    let response = reqwest::Client::new()
        .post(format!("{}/v1/chat/completions", proxy.base_url))
        .json(&chat_request_without_model())
        .send()
        .await
        .expect("proxy request");

    assert_eq!(response.status(), StatusCode::OK);
    let received = upstream.received_requests().await.expect("received");
    let upstream_body: Value = serde_json::from_slice(&received[0].body).expect("json");
    assert_eq!(upstream_body["model"], "gpt-5.5");

    proxy.shutdown().await;
}

#[tokio::test]
async fn chat_completions_without_model_falls_back_to_next_openai_default_model() {
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_partial_json(json!({"model": "gpt-5.5"})))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": {"message": "The model `gpt-5.5` does not exist"}
        })))
        .expect(1)
        .mount(&upstream)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_partial_json(json!({"model": "gpt-5.5-instant"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(openai_text_response("fallback")))
        .expect(1)
        .mount(&upstream)
        .await;

    let proxy = start_proxy_with_providers(&[ProviderSpec {
        id: "fallback-model-openai",
        name: "Fallback Model OpenAI",
        priority: 1,
        server: &upstream,
    }])
    .await;

    let response = reqwest::Client::new()
        .post(format!("{}/v1/chat/completions", proxy.base_url))
        .json(&chat_request_without_model())
        .send()
        .await
        .expect("proxy request");

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.expect("response json");
    assert_eq!(body["choices"][0]["message"]["content"], "fallback");

    let received = upstream.received_requests().await.expect("received");
    let models = received
        .iter()
        .map(|request| {
            serde_json::from_slice::<Value>(&request.body).expect("json")["model"]
                .as_str()
                .expect("model")
                .to_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(models, vec!["gpt-5.5", "gpt-5.5-instant"]);

    proxy.shutdown().await;
}

#[tokio::test]
async fn chat_completions_with_client_model_still_falls_back_to_next_openai_default_model() {
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_partial_json(json!({"model": "gpt-5.5"})))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": {"message": "The model `gpt-5.5` does not exist"}
        })))
        .expect(1)
        .mount(&upstream)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_partial_json(json!({"model": "gpt-5.5-instant"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(openai_text_response("fallback")))
        .expect(1)
        .mount(&upstream)
        .await;

    let proxy = start_proxy_with_providers(&[ProviderSpec {
        id: "fallback-model-ignores-client",
        name: "Fallback Model Ignores Client",
        priority: 1,
        server: &upstream,
    }])
    .await;

    let response = reqwest::Client::new()
        .post(format!("{}/v1/chat/completions", proxy.base_url))
        .json(&chat_request_with_model("client-custom-model"))
        .send()
        .await
        .expect("proxy request");

    assert_eq!(response.status(), StatusCode::OK);
    let received = upstream.received_requests().await.expect("received");
    let models = received
        .iter()
        .map(|request| {
            serde_json::from_slice::<Value>(&request.body).expect("json")["model"]
                .as_str()
                .expect("model")
                .to_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(models, vec!["gpt-5.5", "gpt-5.5-instant"]);

    proxy.shutdown().await;
}

#[tokio::test]
async fn anthropic_messages_to_openai_provider_uses_default_model() {
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(openai_text_response("anthropic default")),
        )
        .expect(1)
        .mount(&upstream)
        .await;

    let proxy = start_proxy_with_providers(&[ProviderSpec {
        id: "default-model-from-anthropic",
        name: "Default Model From Anthropic",
        priority: 1,
        server: &upstream,
    }])
    .await;

    let response = reqwest::Client::new()
        .post(format!("{}/v1/messages", proxy.base_url))
        .json(&anthropic_messages_request_with_model("client-slow"))
        .send()
        .await
        .expect("proxy request");

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.expect("response json");
    assert_eq!(body["content"][0]["text"], "anthropic default");
    let received = upstream.received_requests().await.expect("received");
    let upstream_body: Value = serde_json::from_slice(&received[0].body).expect("json");
    assert_eq!(upstream_body["model"], "gpt-5.5");

    proxy.shutdown().await;
}

#[tokio::test]
async fn anthropic_messages_without_model_to_openai_provider_uses_default_model() {
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(openai_text_response("anthropic default")),
        )
        .expect(1)
        .mount(&upstream)
        .await;

    let proxy = start_proxy_with_providers(&[ProviderSpec {
        id: "missing-model-anthropic-openai",
        name: "Missing Model Anthropic OpenAI",
        priority: 1,
        server: &upstream,
    }])
    .await;

    let response = reqwest::Client::new()
        .post(format!("{}/v1/messages", proxy.base_url))
        .json(&anthropic_messages_request_without_model())
        .send()
        .await
        .expect("proxy request");

    assert_eq!(response.status(), StatusCode::OK);
    let received = upstream.received_requests().await.expect("received");
    let upstream_body: Value = serde_json::from_slice(&received[0].body).expect("json");
    assert_eq!(upstream_body["model"], "gpt-5.5");

    proxy.shutdown().await;
}

#[tokio::test]
async fn chat_completions_keeps_upstream_response_model_and_uses_default_model() {
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
    let response_body: Value = response.json().await.expect("response json");
    assert_eq!(response_body["model"], "gpt-5.5-instant");
    let received = upstream.received_requests().await.expect("received");
    let upstream_body: Value = serde_json::from_slice(&received[0].body).expect("json");
    assert_eq!(upstream_body["model"], "gpt-5.5");

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
    let proxy = start_proxy_with_providers_and_timeout(
        &[ProviderSpec {
            id: "timeout-provider",
            name: "Timeout Provider",
            priority: 1,
            server: &upstream,
        }],
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
    let sse = "data: {\"id\":\"chatcmpl-timeout-stream\",\"model\":\"gpt-5.5-instant\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"ok\"},\"finish_reason\":null}]}\n\n\
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
    let proxy = start_proxy_with_providers_and_timeout(
        &[ProviderSpec {
            id: "stream-timeout-provider",
            name: "Stream Timeout Provider",
            priority: 1,
            server: &upstream,
        }],
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
    assert_eq!(upstream_body["model"], "gpt-5.5");
    assert_eq!(upstream_body["messages"][0]["role"], "system");
    assert_eq!(upstream_body["messages"][0]["content"], "You are terse.");
    assert_eq!(upstream_body["messages"][1]["role"], "user");
    assert_eq!(upstream_body["messages"][1]["content"], "ping");
    assert_eq!(upstream_body["max_tokens"], 128);
    assert_eq!(upstream_body["stream"], false);

    proxy.shutdown().await;
}

#[tokio::test]
async fn anthropic_messages_uses_native_anthropic_provider_request_shape() {
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(anthropic_text_response("native")))
        .expect(1)
        .mount(&upstream)
        .await;
    let proxy = start_proxy_with_native_anthropic_provider(&ProviderSpec {
        id: "native-anthropic",
        name: "Native Anthropic",
        priority: 1,
        server: &upstream,
    })
    .await;

    let response = reqwest::Client::new()
        .post(format!("{}/v1/messages", proxy.base_url))
        .json(&anthropic_messages_request())
        .send()
        .await
        .expect("proxy request");

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.expect("response json");
    assert_eq!(body["content"][0]["text"], "native");
    assert_eq!(body["usage"]["input_tokens"], 4);
    assert_eq!(body["usage"]["output_tokens"], 2);

    let received = upstream.received_requests().await.expect("received");
    let request = &received[0];
    let upstream_body: Value = serde_json::from_slice(&request.body).expect("json");
    assert_eq!(upstream_body["model"], "claude-opus-4.7");
    assert_eq!(upstream_body["system"], "You are terse.");
    assert_eq!(upstream_body["messages"][0]["role"], "user");
    assert_eq!(upstream_body["messages"][0]["content"][0]["text"], "ping");
    assert!(upstream_body["messages"]
        .as_array()
        .expect("messages array")
        .iter()
        .all(|message| message["role"] != "system"));
    assert_eq!(
        request
            .headers
            .get("x-api-key")
            .and_then(|value| value.to_str().ok()),
        Some("sk-upstream-test"),
    );
    assert_eq!(
        request
            .headers
            .get("anthropic-version")
            .and_then(|value| value.to_str().ok()),
        Some("2023-06-01"),
    );
    assert_eq!(
        request
            .headers
            .get("user-agent")
            .and_then(|value| value.to_str().ok()),
        Some("claude-cli/2.1.2 (external, cli)"),
    );
    assert_eq!(
        request
            .headers
            .get("x-stainless-lang")
            .and_then(|value| value.to_str().ok()),
        Some("js"),
    );
    assert_eq!(
        request
            .headers
            .get("x-stainless-runtime")
            .and_then(|value| value.to_str().ok()),
        Some("node"),
    );

    proxy.shutdown().await;
}

#[tokio::test]
async fn anthropic_messages_to_native_provider_preserves_parallel_tool_results() {
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(anthropic_text_response("native")))
        .expect(1)
        .mount(&upstream)
        .await;
    let proxy = start_proxy_with_native_anthropic_provider(&ProviderSpec {
        id: "native-anthropic-parallel-tools",
        name: "Native Anthropic Parallel Tools",
        priority: 1,
        server: &upstream,
    })
    .await;

    let response = reqwest::Client::new()
        .post(format!("{}/v1/messages", proxy.base_url))
        .json(&anthropic_messages_parallel_tool_request())
        .send()
        .await
        .expect("proxy request");

    assert_eq!(response.status(), StatusCode::OK);
    let received = upstream.received_requests().await.expect("received");
    let upstream_body: Value = serde_json::from_slice(&received[0].body).expect("json");
    assert_eq!(
        upstream_body["messages"]
            .as_array()
            .expect("messages")
            .len(),
        3
    );
    assert_eq!(
        upstream_body["messages"][1]["content"][0]["id"],
        "toolu_read"
    );
    assert_eq!(
        upstream_body["messages"][1]["content"][1]["id"],
        "toolu_bash"
    );
    assert_eq!(upstream_body["messages"][2]["role"], "user");
    assert_eq!(
        upstream_body["messages"][2]["content"][0]["tool_use_id"],
        "toolu_read"
    );
    assert_eq!(
        upstream_body["messages"][2]["content"][1]["tool_use_id"],
        "toolu_bash"
    );

    proxy.shutdown().await;
}

#[tokio::test]
async fn anthropic_messages_without_model_uses_native_anthropic_default_model() {
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(anthropic_text_response("native default")),
        )
        .expect(1)
        .mount(&upstream)
        .await;
    let proxy = start_proxy_with_native_anthropic_provider(&ProviderSpec {
        id: "native-anthropic-missing-model",
        name: "Native Anthropic Missing Model",
        priority: 1,
        server: &upstream,
    })
    .await;

    let response = reqwest::Client::new()
        .post(format!("{}/v1/messages", proxy.base_url))
        .json(&anthropic_messages_request_without_model())
        .send()
        .await
        .expect("proxy request");

    assert_eq!(response.status(), StatusCode::OK);
    let received = upstream.received_requests().await.expect("received");
    let upstream_body: Value = serde_json::from_slice(&received[0].body).expect("json");
    assert_eq!(upstream_body["model"], "claude-opus-4.7");

    proxy.shutdown().await;
}

#[tokio::test]
async fn anthropic_messages_returns_zero_usage_when_upstream_omits_usage() {
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(openai_text_response_without_usage("pong")),
        )
        .expect(1)
        .mount(&upstream)
        .await;
    let proxy = start_proxy_with_providers(&[ProviderSpec {
        id: "anthropic-missing-usage",
        name: "Anthropic Missing Usage",
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
    assert_eq!(body["content"][0]["text"], "pong");
    assert_eq!(body["usage"]["input_tokens"], 0);
    assert_eq!(body["usage"]["output_tokens"], 0);

    proxy.shutdown().await;
}

#[tokio::test]
async fn anthropic_messages_converts_image_content_to_openai_upstream_shape() {
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(openai_text_response("vision")))
        .expect(1)
        .mount(&upstream)
        .await;
    let proxy = start_proxy_with_providers(&[ProviderSpec {
        id: "anthropic-image",
        name: "Anthropic Image",
        priority: 1,
        server: &upstream,
    }])
    .await;

    let response = reqwest::Client::new()
        .post(format!("{}/v1/messages", proxy.base_url))
        .json(&anthropic_messages_request_with_base64_image())
        .send()
        .await
        .expect("proxy request");

    assert_eq!(response.status(), StatusCode::OK);
    let received = upstream.received_requests().await.expect("received");
    let upstream_body: Value = serde_json::from_slice(&received[0].body).expect("json");
    assert_eq!(
        upstream_body["messages"][0]["content"][0]["text"],
        "describe this"
    );
    assert_eq!(
        upstream_body["messages"][0]["content"][1]["image_url"]["url"],
        "data:image/png;base64,abc123",
    );

    proxy.shutdown().await;
}

#[tokio::test]
async fn anthropic_messages_streaming_converts_image_content_to_openai_upstream_shape() {
    let upstream = MockServer::start().await;
    let sse = "data: {\"id\":\"chatcmpl-anthropic-image-stream\",\"model\":\"gpt-5.5-instant\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\"},\"finish_reason\":null}]}\n\n\
               data: {\"id\":\"chatcmpl-anthropic-image-stream\",\"model\":\"gpt-5.5-instant\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"ok\"},\"finish_reason\":null}]}\n\n\
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
        id: "anthropic-image-stream",
        name: "Anthropic Image Stream",
        priority: 1,
        server: &upstream,
    }])
    .await;

    let response = reqwest::Client::new()
        .post(format!("{}/v1/messages", proxy.base_url))
        .json(&anthropic_messages_stream_request_with_base64_image())
        .send()
        .await
        .expect("proxy request");

    assert_eq!(response.status(), StatusCode::OK);
    let received = upstream.received_requests().await.expect("received");
    let upstream_body: Value = serde_json::from_slice(&received[0].body).expect("json");
    assert_eq!(upstream_body["stream"], true);
    assert_eq!(
        upstream_body["messages"][0]["content"][1]["image_url"]["url"],
        "data:image/png;base64,abc123",
    );

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
    let sse = "data: {\"id\":\"chatcmpl-anthropic-stream\",\"model\":\"gpt-5.5-instant\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\"},\"finish_reason\":null}]}\n\n\
               data: {\"id\":\"chatcmpl-anthropic-stream\",\"model\":\"gpt-5.5-instant\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hel\"},\"finish_reason\":null}]}\n\n\
               data: {\"id\":\"chatcmpl-anthropic-stream\",\"model\":\"gpt-5.5-instant\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"lo\"},\"finish_reason\":null}]}\n\n\
               data: {\"id\":\"chatcmpl-anthropic-stream\",\"model\":\"gpt-5.5-instant\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n\
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
    assert!(body.contains("\"usage\":{\"input_tokens\":0,\"output_tokens\":0}"));
    assert!(body.contains("\"usage\":{\"output_tokens\":0}"));
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
async fn anthropic_messages_stream_passthroughs_native_anthropic_sse() {
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(native_anthropic_sse())
                .insert_header("content-type", "text/event-stream"),
        )
        .expect(1)
        .mount(&upstream)
        .await;
    let proxy = start_proxy_with_native_anthropic_provider(&ProviderSpec {
        id: "native-anthropic-stream",
        name: "Native Anthropic Stream",
        priority: 1,
        server: &upstream,
    })
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
            "event: content_block_delta",
            "\"text\":\"hi\"",
            "event: content_block_stop",
            "event: message_delta",
            "event: message_stop",
        ],
    );
    assert!(!body.contains("data: [DONE]"));
    assert!(!body.contains("chatcmpl"));

    let received = upstream.received_requests().await.expect("received");
    let upstream_body: Value = serde_json::from_slice(&received[0].body).expect("json");
    assert_eq!(upstream_body["stream"], true);
    assert_eq!(upstream_body["messages"][0]["content"][0]["text"], "ping");

    proxy.shutdown().await;
}

#[tokio::test]
async fn anthropic_messages_stream_adds_message_start_when_upstream_omits_role_and_id() {
    let upstream = MockServer::start().await;
    let sse = "data: {\"model\":\"gpt-5.5-instant\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hi\"},\"finish_reason\":null}]}\n\n\
               data: {\"model\":\"gpt-5.5-instant\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n\
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
        id: "anthropic-stream-missing-start",
        name: "Anthropic Stream Missing Start",
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
    assert!(body.contains("\"id\":\"msg_"));
    assert!(body.contains("\"text\":\"Hi\""));
    assert_contains_in_order(
        &body,
        &[
            "event: message_start",
            "event: content_block_start",
            "\"text\":\"Hi\"",
            "event: content_block_stop",
            "event: message_delta",
            "event: message_stop",
        ],
    );
    assert!(!body.contains("\"id\":null"));

    proxy.shutdown().await;
}

#[tokio::test]
async fn anthropic_messages_streams_tool_use_events_in_order() {
    let upstream = MockServer::start().await;
    let sse = r#"data: {"id":"chatcmpl-tool-stream","model":"gpt-5.5-instant","choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}

data: {"id":"chatcmpl-tool-stream","model":"gpt-5.5-instant","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_weather","type":"function","function":{"name":"get_weather","arguments":"{\"city\":"}}]},"finish_reason":null}]}

data: {"id":"chatcmpl-tool-stream","model":"gpt-5.5-instant","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"Tokyo\"}"}}]},"finish_reason":null}]}

data: {"id":"chatcmpl-tool-stream","model":"gpt-5.5-instant","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}

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
async fn anthropic_messages_stream_error_returns_sse_error_frame_without_socket_reset() {
    let upstream = MockServer::start().await;
    let sse = "data: {\"id\":\"chatcmpl-stream-error\",\"model\":\"gpt-5.5-instant\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\"},\"finish_reason\":null}]}\n\n\
               data: {not-json}\n\n";
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
        id: "anthropic-stream-error",
        name: "Anthropic Stream Error",
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
    let body = response
        .text()
        .await
        .expect("sse text must not reset socket");
    assert_contains_in_order(&body, &["event: message_start", "event: error"]);
    assert!(body.contains("\"type\":\"error\""));
    assert!(body.contains("\"type\":\"api_error\""));

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
    assert_eq!(upstream_body["model"], "gpt-5.5");
    assert_eq!(upstream_body["messages"][0]["content"], "ping");
    assert_eq!(upstream_body["stream"], false);

    proxy.shutdown().await;
}

#[tokio::test]
async fn chat_completions_uses_native_anthropic_provider_request_shape() {
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(anthropic_text_response("native")))
        .expect(1)
        .mount(&upstream)
        .await;
    let proxy = start_proxy_with_native_anthropic_provider(&ProviderSpec {
        id: "chat-native-anthropic",
        name: "Chat Native Anthropic",
        priority: 1,
        server: &upstream,
    })
    .await;

    let response = reqwest::Client::new()
        .post(format!("{}/v1/chat/completions", proxy.base_url))
        .json(&chat_request(false))
        .send()
        .await
        .expect("proxy request");

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.expect("response json");
    assert_eq!(body["choices"][0]["message"]["content"], "native");
    assert_eq!(body["usage"]["prompt_tokens"], 4);
    assert_eq!(body["usage"]["completion_tokens"], 2);

    let received = upstream.received_requests().await.expect("received");
    let upstream_body: Value = serde_json::from_slice(&received[0].body).expect("json");
    assert_eq!(upstream_body["model"], "claude-opus-4.7");
    assert_eq!(upstream_body["messages"][0]["role"], "user");
    assert_eq!(upstream_body["messages"][0]["content"][0]["text"], "ping");
    assert_eq!(upstream_body["stream"], Value::Null);
    assert_eq!(
        received[0]
            .headers
            .get("user-agent")
            .and_then(|value| value.to_str().ok()),
        Some("claude-cli/2.1.2 (external, cli)"),
    );
    assert_eq!(
        received[0]
            .headers
            .get("x-stainless-lang")
            .and_then(|value| value.to_str().ok()),
        Some("js"),
    );

    proxy.shutdown().await;
}

#[tokio::test]
async fn chat_completions_forwards_multimodal_content_to_upstream() {
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(openai_text_response("vision")))
        .expect(1)
        .mount(&upstream)
        .await;
    let proxy = start_proxy_with_providers(&[ProviderSpec {
        id: "openai-image",
        name: "OpenAI Image",
        priority: 1,
        server: &upstream,
    }])
    .await;

    let response = reqwest::Client::new()
        .post(format!("{}/v1/chat/completions", proxy.base_url))
        .json(&chat_request_with_image())
        .send()
        .await
        .expect("proxy request");

    assert_eq!(response.status(), StatusCode::OK);
    let received = upstream.received_requests().await.expect("received");
    let upstream_body: Value = serde_json::from_slice(&received[0].body).expect("json");
    assert_eq!(
        upstream_body["messages"][0]["content"][0]["text"],
        "describe this"
    );
    assert_eq!(
        upstream_body["messages"][0]["content"][1]["image_url"]["url"],
        "data:image/png;base64,abc123",
    );
    assert_eq!(
        upstream_body["messages"][0]["content"][1]["image_url"]["detail"],
        "high",
    );

    proxy.shutdown().await;
}

#[tokio::test]
async fn chat_completions_streams_multimodal_content_to_upstream() {
    let upstream = MockServer::start().await;
    let sse = "data: {\"id\":\"chatcmpl-image-stream\",\"model\":\"gpt-5.5-instant\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"ok\"},\"finish_reason\":null}]}\n\n\
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
        id: "openai-image-stream",
        name: "OpenAI Image Stream",
        priority: 1,
        server: &upstream,
    }])
    .await;

    let response = reqwest::Client::new()
        .post(format!("{}/v1/chat/completions", proxy.base_url))
        .json(&chat_stream_request_with_image())
        .send()
        .await
        .expect("proxy request");

    assert_eq!(response.status(), StatusCode::OK);
    let received = upstream.received_requests().await.expect("received");
    let upstream_body: Value = serde_json::from_slice(&received[0].body).expect("json");
    assert_eq!(upstream_body["stream"], true);
    assert_eq!(
        upstream_body["messages"][0]["content"][1]["image_url"]["url"],
        "data:image/png;base64,abc123",
    );

    proxy.shutdown().await;
}

#[tokio::test]
async fn chat_completions_writes_request_log_for_monitoring() {
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(openai_text_response("logged")))
        .expect(1)
        .mount(&upstream)
        .await;
    let (_dir, db) = request_log_database_with_provider("monitor-provider");
    let proxy = start_proxy_with_providers_timeout_and_request_log(
        &[ProviderSpec {
            id: "monitor-provider",
            name: "Monitor Provider",
            priority: 1,
            server: &upstream,
        }],
        None,
        Some(db.clone()),
    )
    .await;

    let response = reqwest::Client::new()
        .post(format!("{}/v1/chat/completions", proxy.base_url))
        .json(&chat_request(false))
        .send()
        .await
        .expect("proxy request");

    assert_eq!(response.status(), StatusCode::OK);
    let logs = RequestLogRepository::new(db)
        .list_recent(1)
        .expect("request logs");
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].provider_id, "monitor-provider");
    assert_eq!(logs[0].model, "gpt-5.5-instant");
    assert_eq!(logs[0].status, "ok");
    assert_eq!(logs[0].total_tokens, Some(6));
    assert!(!logs[0].stream);

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
    let sse = "data: {\"id\":\"chatcmpl-stream\",\"model\":\"gpt-5.5-instant\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"Hel\"},\"finish_reason\":null}]}\n\n\
               data: {\"id\":\"chatcmpl-stream\",\"model\":\"gpt-5.5-instant\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"lo\"},\"finish_reason\":null}]}\n\n\
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
async fn chat_completions_stream_converts_native_anthropic_sse_to_openai_sse() {
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(native_anthropic_sse())
                .insert_header("content-type", "text/event-stream"),
        )
        .expect(1)
        .mount(&upstream)
        .await;
    let proxy = start_proxy_with_native_anthropic_provider(&ProviderSpec {
        id: "chat-native-anthropic-stream",
        name: "Chat Native Anthropic Stream",
        priority: 1,
        server: &upstream,
    })
    .await;

    let response = reqwest::Client::new()
        .post(format!("{}/v1/chat/completions", proxy.base_url))
        .json(&chat_request(true))
        .send()
        .await
        .expect("proxy request");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.text().await.expect("sse text");
    assert_contains_in_order(
        &body,
        &[
            "data: {",
            "\"role\":\"assistant\"",
            "\"content\":\"hi\"",
            "data: [DONE]",
        ],
    );
    assert!(body.contains("\"object\":\"chat.completion.chunk\""));
    assert!(!body.contains("event: message_start"));

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

#[tokio::test]
async fn chat_completions_retries_after_provider_bad_request_and_uses_next_provider() {
    let primary = MockServer::start().await;
    let backup = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(400)
                .set_body_json(json!({"error": {"message": "request shape not supported here"}})),
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
            id: "primary-bad-request",
            name: "Primary Bad Request",
            priority: 1,
            server: &primary,
        },
        ProviderSpec {
            id: "backup-after-bad-request",
            name: "Backup After Bad Request",
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
        .get("primary-bad-request")
        .await
        .expect("primary provider");
    assert_eq!(first.state.health().await, HealthStatus::Degraded);

    proxy.shutdown().await;
}

#[tokio::test]
async fn chat_completions_records_switch_history_after_503_failover() {
    let primary = MockServer::start().await;
    let backup = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(503).set_body_string("temporarily unavailable"))
        .expect(1)
        .mount(&primary)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(openai_text_response("backup")))
        .expect(1)
        .mount(&backup)
        .await;
    let (_dir, db) = monitoring_database_with_providers(&["primary-503", "backup-after-503"]);
    let proxy = start_proxy_with_providers_timeout_and_request_log(
        &[
            ProviderSpec {
                id: "primary-503",
                name: "Primary 503",
                priority: 1,
                server: &primary,
            },
            ProviderSpec {
                id: "backup-after-503",
                name: "Backup After 503",
                priority: 2,
                server: &backup,
            },
        ],
        None,
        Some(db.clone()),
    )
    .await;

    let response = reqwest::Client::new()
        .post(format!("{}/v1/chat/completions", proxy.base_url))
        .json(&chat_request(false))
        .send()
        .await
        .expect("proxy request");

    assert_eq!(response.status(), StatusCode::OK);
    let primary_wrapper = proxy
        .manager
        .get("primary-503")
        .await
        .expect("primary provider");
    assert_eq!(primary_wrapper.state.health().await, HealthStatus::Degraded);
    let events = SwitchHistoryRepository::new(db)
        .list_recent(1)
        .expect("switch events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].from_provider.as_deref(), Some("primary-503"));
    assert_eq!(events[0].to_provider, "backup-after-503");
    assert_eq!(events[0].strategy, "priority");
    assert_eq!(events[0].reason, "upstream_503");
    assert_eq!(events[0].attempts, 2);

    proxy.shutdown().await;
}
