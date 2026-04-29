//! Integration tests for the local proxy server scaffold.
//!
//! Pin the three behaviors clients depend on:
//! 1. binding port `0` exposes the OS-assigned port via `local_addr`,
//! 2. `/healthz` responds `200 ok` once the server is running,
//! 3. resolving the shutdown future causes `serve` to return cleanly.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use ccuse_desktop_lib::auth::key_store;
use ccuse_desktop_lib::commands::model_mapping::ModelMappingHandle;
use ccuse_desktop_lib::commands::switch::SwitchEngineHandle;
use ccuse_desktop_lib::converter::ModelMapping;
use ccuse_desktop_lib::providers::ProviderManager;
use ccuse_desktop_lib::proxy::{ProxyAppState, ProxyServer, ServerError};
use ccuse_desktop_lib::switch::SwitchEngine;
use serde_json::Value;
use tokio::sync::{oneshot, RwLock};
use tokio::task::JoinHandle;

fn loopback_zero() -> SocketAddr {
    "127.0.0.1:0"
        .parse()
        .expect("loopback string is a valid SocketAddr")
}

fn test_proxy_state() -> ProxyAppState {
    let manager = Arc::new(ProviderManager::new());
    let engine: SwitchEngineHandle = Arc::new(SwitchEngine::new(Arc::clone(&manager)));
    let model_mapping: ModelMappingHandle = Arc::new(RwLock::new(ModelMapping::new()));
    ProxyAppState::new(engine, model_mapping, manager)
}

/// Spin up a proxy server bound to an ephemeral port and return
/// `(base_url, shutdown_tx, serve_handle)`. Sleeps briefly so the
/// listener has time to start accepting before the caller fires
/// requests at it.
async fn start_test_server() -> (
    String,
    oneshot::Sender<()>,
    JoinHandle<Result<(), ServerError>>,
) {
    let server = ProxyServer::bind(loopback_zero())
        .await
        .expect("bind to ephemeral port should succeed");
    let base = format!("http://{}", server.local_addr());
    let (tx, rx) = oneshot::channel::<()>();
    let handle = tokio::spawn(server.serve_with_shutdown(test_proxy_state(), async move {
        let _ = rx.await;
    }));
    tokio::time::sleep(Duration::from_millis(50)).await;
    (base, tx, handle)
}

async fn shutdown_test_server(
    tx: oneshot::Sender<()>,
    handle: JoinHandle<Result<(), ServerError>>,
) {
    let _ = tx.send(());
    let join = tokio::time::timeout(Duration::from_secs(2), handle)
        .await
        .expect("server should shut down within 2s")
        .expect("serve task should not panic");
    assert!(join.is_ok(), "serve must return Ok after shutdown");
}

#[tokio::test]
async fn bind_to_port_zero_resolves_real_loopback_port() {
    let server = ProxyServer::bind(loopback_zero())
        .await
        .expect("bind to 127.0.0.1:0 should succeed on a healthy host");

    let addr = server.local_addr();
    assert!(addr.ip().is_loopback(), "should bind to loopback only");
    assert_ne!(addr.port(), 0, "OS must replace 0 with a real port");
}

#[tokio::test]
async fn healthz_endpoint_responds_with_ok() {
    let server = ProxyServer::bind(loopback_zero())
        .await
        .expect("bind succeeds");
    let base = format!("http://{}", server.local_addr());
    let (tx, rx) = oneshot::channel::<()>();

    let serve_handle = tokio::spawn(server.serve_with_shutdown(test_proxy_state(), async move {
        let _ = rx.await;
    }));

    // Brief yield so the server has a chance to start accepting.
    tokio::time::sleep(Duration::from_millis(50)).await;

    let response = reqwest::get(format!("{base}/healthz"))
        .await
        .expect("healthz request should reach the server");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let body = response.text().await.expect("body decodes as utf-8");
    assert_eq!(body, "ok");

    let _ = tx.send(());
    let serve_result = tokio::time::timeout(Duration::from_secs(2), serve_handle)
        .await
        .expect("server should shut down within 2s")
        .expect("join handle should not panic");
    assert!(
        serve_result.is_ok(),
        "serve should return Ok after shutdown"
    );
}

#[tokio::test]
async fn serve_returns_after_shutdown_signal() {
    let server = ProxyServer::bind(loopback_zero())
        .await
        .expect("bind succeeds");
    let (tx, rx) = oneshot::channel::<()>();

    let serve_handle = tokio::spawn(server.serve_with_shutdown(test_proxy_state(), async move {
        let _ = rx.await;
    }));

    // Server is running. Fire shutdown immediately.
    tx.send(()).expect("shutdown receiver should still exist");

    let result = tokio::time::timeout(Duration::from_secs(2), serve_handle)
        .await
        .expect("graceful shutdown must complete within 2s");
    assert!(result.is_ok(), "serve task should not panic");
    assert!(
        result.expect("join ok").is_ok(),
        "serve should exit Ok after shutdown",
    );
}

#[tokio::test]
async fn bind_with_fallback_succeeds_with_single_attempt_on_zero() {
    // start=0 lets the OS allocate; one attempt always succeeds on a healthy host.
    let server = ProxyServer::bind_with_fallback(0, 1)
        .await
        .expect("OS should hand out an ephemeral port for start=0");
    assert!(server.local_addr().ip().is_loopback());
    assert_ne!(server.local_addr().port(), 0);
}

#[tokio::test]
async fn list_models_returns_empty_data_array() {
    let (base, tx, handle) = start_test_server().await;
    let response = reqwest::get(format!("{base}/v1/models"))
        .await
        .expect("models request should reach the server");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let body: Value = response.json().await.expect("body decodes as JSON");
    assert_eq!(body["object"], "list");
    assert!(
        body["data"].as_array().is_some_and(Vec::is_empty),
        "data array should be empty until ProviderManager wires real models",
    );
    shutdown_test_server(tx, handle).await;
}

#[tokio::test]
async fn chat_completions_stub_returns_503_with_openai_shaped_error() {
    let (base, tx, handle) = start_test_server().await;
    let response = reqwest::Client::new()
        .post(format!("{base}/v1/chat/completions"))
        .json(&serde_json::json!({"model": "gpt-4o", "messages": []}))
        .send()
        .await
        .expect("chat completions request should reach the server");
    assert_eq!(response.status(), reqwest::StatusCode::SERVICE_UNAVAILABLE);
    let body: Value = response.json().await.expect("body decodes as JSON");
    assert_eq!(body["error"]["type"], "providers_not_configured");
    assert!(body["error"]["message"]
        .as_str()
        .is_some_and(|s| !s.is_empty()));
    shutdown_test_server(tx, handle).await;
}

#[tokio::test]
async fn anthropic_messages_stub_returns_503_with_openai_shaped_error() {
    let (base, tx, handle) = start_test_server().await;
    let response = reqwest::Client::new()
        .post(format!("{base}/v1/messages"))
        .json(&serde_json::json!({"model": "claude-3-5-sonnet", "messages": []}))
        .send()
        .await
        .expect("messages request should reach the server");
    assert_eq!(response.status(), reqwest::StatusCode::SERVICE_UNAVAILABLE);
    let body: Value = response.json().await.expect("body decodes as JSON");
    assert_eq!(body["error"]["type"], "providers_not_configured");
    shutdown_test_server(tx, handle).await;
}

#[tokio::test]
async fn cors_preflight_from_loopback_origin_is_allowed() {
    let (base, tx, handle) = start_test_server().await;
    let response = reqwest::Client::new()
        .request(
            reqwest::Method::OPTIONS,
            format!("{base}/v1/chat/completions"),
        )
        .header("Origin", "http://127.0.0.1:5173")
        .header("Access-Control-Request-Method", "POST")
        .header(
            "Access-Control-Request-Headers",
            "authorization,content-type",
        )
        .send()
        .await
        .expect("preflight request should reach the server");
    assert!(
        response.status().is_success(),
        "preflight should be 2xx, got {}",
        response.status(),
    );
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok()),
        Some("http://127.0.0.1:5173"),
    );
    shutdown_test_server(tx, handle).await;
}

#[tokio::test]
async fn cors_preflight_from_foreign_origin_is_rejected() {
    let (base, tx, handle) = start_test_server().await;
    let response = reqwest::Client::new()
        .request(
            reqwest::Method::OPTIONS,
            format!("{base}/v1/chat/completions"),
        )
        .header("Origin", "https://evil.example.com")
        .header("Access-Control-Request-Method", "POST")
        .send()
        .await
        .expect("preflight request should reach the server");
    // tower-http's policy returns the response without ACAO when the
    // origin doesn't match — clients see this as "CORS forbidden".
    assert!(
        response
            .headers()
            .get("access-control-allow-origin")
            .is_none(),
        "foreign origin must not receive an Access-Control-Allow-Origin header",
    );
    shutdown_test_server(tx, handle).await;
}

/// Spin up a proxy with the auth middleware mounted; used by the
/// T1.0.1.13 integration tests. Returns
/// `(base_url, expected_key, shutdown_tx, serve_handle)`.
async fn start_authenticated_test_server() -> (
    String,
    String,
    oneshot::Sender<()>,
    JoinHandle<Result<(), ServerError>>,
) {
    let server = ProxyServer::bind(loopback_zero())
        .await
        .expect("bind to ephemeral port should succeed");
    let base = format!("http://{}", server.local_addr());
    let key = "sk-local-integration-test-key".to_owned();
    let store = key_store(key.clone());
    let (tx, rx) = oneshot::channel::<()>();
    let handle =
        tokio::spawn(
            server.serve_with_auth_and_shutdown(store, test_proxy_state(), async move {
                let _ = rx.await;
            }),
        );
    tokio::time::sleep(Duration::from_millis(50)).await;
    (base, key, tx, handle)
}

#[tokio::test]
async fn auth_v1_models_returns_401_when_no_key_provided() {
    let (base, _key, tx, handle) = start_authenticated_test_server().await;
    let response = reqwest::get(format!("{base}/v1/models"))
        .await
        .expect("request reaches server");
    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    let body: Value = response.json().await.expect("body decodes");
    assert_eq!(body["error"]["type"], "unauthorized");
    shutdown_test_server(tx, handle).await;
}

#[tokio::test]
async fn auth_v1_chat_completions_accepts_bearer_authorization() {
    let (base, key, tx, handle) = start_authenticated_test_server().await;
    let response = reqwest::Client::new()
        .post(format!("{base}/v1/chat/completions"))
        .bearer_auth(&key)
        .json(&serde_json::json!({"model": "gpt-4o", "messages": []}))
        .send()
        .await
        .expect("request reaches server");
    // Past the auth gate the handler is still the 503 stub.
    assert_eq!(response.status(), reqwest::StatusCode::SERVICE_UNAVAILABLE);
    let body: Value = response.json().await.expect("body decodes");
    assert_eq!(body["error"]["type"], "providers_not_configured");
    shutdown_test_server(tx, handle).await;
}

#[tokio::test]
async fn auth_v1_messages_accepts_x_api_key_header() {
    let (base, key, tx, handle) = start_authenticated_test_server().await;
    let response = reqwest::Client::new()
        .post(format!("{base}/v1/messages"))
        .header("x-api-key", &key)
        .json(&serde_json::json!({"model": "claude", "messages": []}))
        .send()
        .await
        .expect("request reaches server");
    assert_eq!(response.status(), reqwest::StatusCode::SERVICE_UNAVAILABLE);
    let body: Value = response.json().await.expect("body decodes");
    assert_eq!(body["error"]["type"], "providers_not_configured");
    shutdown_test_server(tx, handle).await;
}

#[tokio::test]
async fn auth_rejects_wrong_key_with_401() {
    let (base, _key, tx, handle) = start_authenticated_test_server().await;
    let response = reqwest::Client::new()
        .get(format!("{base}/v1/models"))
        .bearer_auth("sk-local-wrong-key-0000000000000000000000")
        .send()
        .await
        .expect("request reaches server");
    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    shutdown_test_server(tx, handle).await;
}

#[tokio::test]
async fn auth_does_not_apply_to_healthz() {
    let (base, _key, tx, handle) = start_authenticated_test_server().await;
    let response = reqwest::get(format!("{base}/healthz"))
        .await
        .expect("request reaches server");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let body = response.text().await.expect("body decodes");
    assert_eq!(body, "ok");
    shutdown_test_server(tx, handle).await;
}

#[tokio::test]
async fn bind_with_fallback_skips_busy_port_and_finds_next() {
    // Hold one loopback port for the duration of the test, then ask
    // bind_with_fallback to start exactly there. The first probe must fail,
    // and the prober must walk up to a higher port.
    let occupier = ProxyServer::bind(loopback_zero())
        .await
        .expect("occupier bind should succeed");
    let busy_port = occupier.local_addr().port();

    let server = ProxyServer::bind_with_fallback(busy_port, 100)
        .await
        .expect("prober should find an available port within 100 attempts");

    assert_ne!(
        server.local_addr().port(),
        busy_port,
        "prober must not re-use the occupied port",
    );
    // Keep `occupier` alive until the assertion: dropping it earlier
    // would release the port and break the test's invariant.
    drop(occupier);
}
