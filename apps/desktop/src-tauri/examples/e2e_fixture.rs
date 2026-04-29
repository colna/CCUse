//! Playwright fixture for T1.0.6.20.
//!
//! Starts a real `CCUse` proxy, a mock OpenAI-compatible upstream, and a
//! small control API that the browser-side mocked Tauri IPC calls.

use std::io::{self, Write};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use ccuse_desktop_lib::auth::key_store;
use ccuse_desktop_lib::commands::monitor::query_metrics_timeseries;
use ccuse_desktop_lib::converter::ModelMapping;
use ccuse_desktop_lib::crypto::MasterKey;
use ccuse_desktop_lib::db::{open_database, run_migrations, Database};
use ccuse_desktop_lib::providers::{Provider, ProviderInput, ProviderManager, ProviderRepository};
use ccuse_desktop_lib::proxy::{ProxyAppState, ProxyServer};
use ccuse_desktop_lib::switch::SwitchEngine;
use serde::Serialize;
use serde_json::{json, Value};
use tempfile::TempDir;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;

const LOCAL_API_KEY: &str = "sk-local-e2e-fixture";

#[derive(Clone)]
struct FixtureState {
    repo: Arc<ProviderRepository>,
    manager: Arc<ProviderManager>,
    db: Database,
}

#[derive(Serialize)]
struct FixtureConfig {
    control_base_url: String,
    proxy_base_url: String,
    api_key: String,
    mock_provider_base_url: String,
}

fn loopback_zero() -> SocketAddr {
    "127.0.0.1:0"
        .parse()
        .expect("loopback string is a valid SocketAddr")
}

fn internal_error(err: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

async fn list_providers(
    State(state): State<FixtureState>,
) -> Result<Json<Vec<Provider>>, (StatusCode, String)> {
    state.repo.list().map(Json).map_err(internal_error)
}

async fn add_provider(
    State(state): State<FixtureState>,
    Json(input): Json<ProviderInput>,
) -> Result<Json<Provider>, (StatusCode, String)> {
    let provider = state.repo.add(&input).map_err(internal_error)?;
    state
        .manager
        .reload_from_repository(state.repo.as_ref())
        .await
        .map_err(internal_error)?;
    Ok(Json(provider))
}

async fn metrics(
    State(state): State<FixtureState>,
) -> Result<Json<Vec<ccuse_desktop_lib::commands::monitor::MetricsBucket>>, (StatusCode, String)> {
    query_metrics_timeseries(&state.db)
        .map(Json)
        .map_err(internal_error)
}

async fn upstream_models() -> Json<Value> {
    Json(json!({
        "object": "list",
        "data": [{"id": "gpt-4o", "object": "model"}]
    }))
}

async fn upstream_chat(Json(_body): Json<Value>) -> Json<Value> {
    Json(json!({
        "id": "chatcmpl-playwright-e2e",
        "object": "chat.completion",
        "created": 1_700_000_000_u64,
        "model": "gpt-4o",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "pong"},
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 4, "completion_tokens": 2, "total_tokens": 6}
    }))
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let db = open_database(temp_dir.path().join("e2e.db"))?;
    run_migrations(&db)?;

    let manager = Arc::new(ProviderManager::new());
    let engine = Arc::new(SwitchEngine::new(Arc::clone(&manager)));
    let model_mapping = Arc::new(RwLock::new(ModelMapping::new()));
    let proxy_state =
        ProxyAppState::new(engine, model_mapping, Arc::clone(&manager)).with_monitoring(db.clone());
    let proxy = ProxyServer::bind(loopback_zero()).await?;
    let proxy_base_url = format!("http://{}", proxy.local_addr());
    let proxy_handle = tokio::spawn(proxy.serve_with_auth_and_shutdown(
        key_store(LOCAL_API_KEY.to_owned()),
        proxy_state,
        std::future::pending::<()>(),
    ));

    let upstream_app = Router::new()
        .route("/v1/models", get(upstream_models))
        .route("/v1/chat/completions", post(upstream_chat));
    let upstream_listener = tokio::net::TcpListener::bind(loopback_zero()).await?;
    let mock_provider_base_url = format!("http://{}", upstream_listener.local_addr()?);
    let upstream_handle = tokio::spawn(async move {
        if let Err(err) = axum::serve(upstream_listener, upstream_app).await {
            eprintln!("upstream fixture stopped: {err}");
        }
    });

    let key = Arc::new(MasterKey::generate()?);
    let state = FixtureState {
        repo: Arc::new(ProviderRepository::new(db.clone(), key)),
        manager,
        db,
    };
    let control_app = Router::new()
        .route("/providers", get(list_providers).post(add_provider))
        .route("/metrics", get(metrics))
        .layer(CorsLayer::permissive())
        .with_state(state);
    let control_listener = tokio::net::TcpListener::bind(loopback_zero()).await?;
    let control_base_url = format!("http://{}", control_listener.local_addr()?);
    let control_handle = tokio::spawn(async move {
        if let Err(err) = axum::serve(control_listener, control_app).await {
            eprintln!("control fixture stopped: {err}");
        }
    });

    let config = FixtureConfig {
        control_base_url,
        proxy_base_url,
        api_key: LOCAL_API_KEY.to_owned(),
        mock_provider_base_url,
    };
    println!("{}", serde_json::to_string(&config)?);
    io::stdout().flush()?;

    tokio::time::sleep(Duration::from_secs(120)).await;
    proxy_handle.abort();
    upstream_handle.abort();
    control_handle.abort();
    Ok(())
}
