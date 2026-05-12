//! Runtime supervisor for the local proxy.
//!
//! Holds the live [`ProxyServer`] handle, the issued protocol key set,
//! and the shutdown channel — i.e. everything T1.0.1.12 commands need
//! to mutate. Pure-Rust API so tests don't have to spin up `Tauri`.

use std::net::SocketAddr;
use std::sync::Arc;

use serde::Serialize;
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

use crate::auth::{generate_local_api_key, key_store, KeyStore, LocalApiKeySet};
use crate::commands::switch::SwitchEngineHandle;
use crate::db::Database;
use crate::providers::ProviderManager;
use crate::switch::SwitchEngine;

use super::server::{ProxyAppState, ProxyServer, ServerError};

/// Default loopback start port for the proxy. Probed range is
/// `[8787, 8887)` (100 attempts) — matches the desktop spec in
/// `docs/产品技术文档.md` §运行时配置.
pub const DEFAULT_PROXY_PORT: u16 = 8787;
/// Probe budget when [`DEFAULT_PROXY_PORT`] is busy.
pub const DEFAULT_PROXY_ATTEMPTS: u16 = 100;

/// Protocol-specific local endpoint returned to the UI / clipboard.
#[derive(Clone, Serialize, PartialEq, Eq)]
pub struct LocalApiEndpointConfig {
    /// Base URL that should be pasted into clients for this protocol.
    pub base_url: String,
    /// Protocol-scoped `sk-local-{32}` token issued at start / regenerate.
    pub api_key: String,
}

impl std::fmt::Debug for LocalApiEndpointConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalApiEndpointConfig")
            .field("base_url", &self.base_url)
            .field("api_key", &"<redacted>")
            .finish()
    }
}

/// Snapshot returned to the UI / clipboard. Top-level `base_url` /
/// `api_key` remain for old UI/tray callers; new surfaces should use
/// the protocol-specific `openai` / `anthropic` fields.
#[derive(Clone, Serialize, PartialEq, Eq)]
pub struct LocalApiConfig {
    /// Legacy root URL, e.g. `http://127.0.0.1:8787`.
    pub base_url: String,
    /// Legacy key alias for OpenAI-compatible clients.
    pub api_key: String,
    pub openai: LocalApiEndpointConfig,
    pub anthropic: LocalApiEndpointConfig,
}

impl std::fmt::Debug for LocalApiConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalApiConfig")
            .field("base_url", &self.base_url)
            .field("api_key", &"<redacted>")
            .field("openai", &self.openai)
            .field("anthropic", &self.anthropic)
            .finish()
    }
}

/// Errors surfaced through the runtime / Tauri command layer.
#[derive(thiserror::Error, Debug)]
pub enum RuntimeError {
    #[error("proxy is not running")]
    NotRunning,
    #[error("proxy already running on {0}")]
    AlreadyRunning(SocketAddr),
    #[error(transparent)]
    Server(#[from] ServerError),
    #[error("serve task ended with error: {0}")]
    Serve(String),
    #[error("serve task panicked: {0}")]
    Panic(String),
}

/// Live state when the proxy is up. `None` between `stop` and the
/// next `start`.
struct RunningState {
    addr: SocketAddr,
    api_keys: LocalApiKeySet,
    /// Shared with the auth middleware. Updating this in place is
    /// what makes `regenerate_api_key` not require a server bounce.
    key_store: KeyStore,
    shutdown: oneshot::Sender<()>,
    handle: JoinHandle<Result<(), ServerError>>,
}

impl std::fmt::Debug for RunningState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunningState")
            .field("addr", &self.addr)
            .field("api_keys", &"<redacted>")
            .finish_non_exhaustive()
    }
}

/// Owns the long-lived proxy state. Cheap to clone via `Arc` at the
/// Tauri layer (`tauri::State<Arc<ProxyRuntime>>`).
#[derive(Debug)]
pub struct ProxyRuntime {
    inner: Mutex<Option<RunningState>>,
    fallback_start: u16,
    fallback_attempts: u16,
    state: ProxyAppState,
}

impl Default for ProxyRuntime {
    fn default() -> Self {
        Self::new(DEFAULT_PROXY_PORT, DEFAULT_PROXY_ATTEMPTS)
    }
}

impl ProxyRuntime {
    /// Build with explicit port-probe parameters. Tests pass `(0, 1)`
    /// to let the OS allocate; production uses the defaults.
    #[must_use]
    pub fn new(fallback_start: u16, fallback_attempts: u16) -> Self {
        let manager = Arc::new(ProviderManager::new());
        let engine: SwitchEngineHandle = Arc::new(SwitchEngine::new(Arc::clone(&manager)));
        Self::with_dependencies(fallback_start, fallback_attempts, engine, manager)
    }

    #[must_use]
    pub fn with_dependencies(
        fallback_start: u16,
        fallback_attempts: u16,
        engine: SwitchEngineHandle,
        manager: Arc<ProviderManager>,
    ) -> Self {
        Self {
            inner: Mutex::new(None),
            fallback_start,
            fallback_attempts,
            state: ProxyAppState::new(engine, manager),
        }
    }

    #[must_use]
    pub fn with_dependencies_and_request_log(
        fallback_start: u16,
        fallback_attempts: u16,
        engine: SwitchEngineHandle,
        manager: Arc<ProviderManager>,
        db: Database,
    ) -> Self {
        let mut runtime =
            Self::with_dependencies(fallback_start, fallback_attempts, engine, manager);
        runtime.state = runtime.state.with_request_log(db);
        runtime
    }

    #[must_use]
    pub fn with_dependencies_and_monitoring(
        fallback_start: u16,
        fallback_attempts: u16,
        engine: SwitchEngineHandle,
        manager: Arc<ProviderManager>,
        db: Database,
    ) -> Self {
        let mut runtime =
            Self::with_dependencies(fallback_start, fallback_attempts, engine, manager);
        runtime.state = runtime.state.with_monitoring(db);
        runtime
    }

    /// Start the proxy if it's not already running. Generates fresh
    /// protocol-scoped `sk-local-…` keys and returns the config snapshot.
    pub async fn start(&self) -> Result<LocalApiConfig, RuntimeError> {
        let mut guard = self.inner.lock().await;
        if let Some(state) = guard.as_ref() {
            return Err(RuntimeError::AlreadyRunning(state.addr));
        }
        let server =
            ProxyServer::bind_with_fallback(self.fallback_start, self.fallback_attempts).await?;
        let addr = server.local_addr();
        if addr.port() != self.fallback_start && self.fallback_start != 0 {
            eprintln!(
                "CCUse: preferred port {} busy, fell back to {}",
                self.fallback_start,
                addr.port(),
            );
        }
        let api_keys = generate_protocol_key_set();
        let store = key_store(api_keys.clone());
        let (shutdown, rx) = oneshot::channel::<()>();
        let handle = tokio::spawn(server.serve_with_auth_and_shutdown(
            store.clone(),
            self.state.clone(),
            async move {
                let _ = rx.await;
            },
        ));
        let config = local_api_config(addr, &api_keys);
        *guard = Some(RunningState {
            addr,
            api_keys,
            key_store: store,
            shutdown,
            handle,
        });
        Ok(config)
    }

    /// Snapshot the current config, or `None` if the proxy isn't up.
    pub async fn current_config(&self) -> Option<LocalApiConfig> {
        let guard = self.inner.lock().await;
        guard
            .as_ref()
            .map(|state| local_api_config(state.addr, &state.api_keys))
    }

    /// Replace the in-memory keys with freshly generated ones. Does
    /// not bounce the server — existing connections keep their
    /// (already-authorised) request handles. `T1.0.1.13` middleware
    /// reads the current key per-request, so the swap takes effect
    /// on the next inbound call.
    pub async fn regenerate_api_key(&self) -> Result<LocalApiConfig, RuntimeError> {
        let mut guard = self.inner.lock().await;
        let state = guard.as_mut().ok_or(RuntimeError::NotRunning)?;
        let fresh = generate_protocol_key_set();
        // Push the new key into the auth keystore *before* swapping
        // the in-runtime copy: a request that races with rotation
        // either sees the old expected key (still valid) or the new
        // one — never an empty / inconsistent state.
        {
            let mut guard = state
                .key_store
                .write()
                .map_err(|_| RuntimeError::Serve("auth keystore lock poisoned".into()))?;
            *guard = fresh.clone();
        }
        state.api_keys = fresh;
        Ok(local_api_config(state.addr, &state.api_keys))
    }

    /// Stop the running proxy, then start a fresh one. Reuses the
    /// fallback range so a freed port is preferred. Returns the new
    /// config (port may differ; key is rotated).
    pub async fn restart(&self) -> Result<LocalApiConfig, RuntimeError> {
        self.stop().await?;
        self.start().await
    }

    /// Gracefully stop the proxy. Idempotent: stopping when already
    /// stopped returns `Ok(())`.
    pub async fn stop(&self) -> Result<(), RuntimeError> {
        let mut guard = self.inner.lock().await;
        let Some(state) = guard.take() else {
            return Ok(());
        };
        let _ = state.shutdown.send(());
        match state.handle.await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(err)) => Err(RuntimeError::Serve(err.to_string())),
            Err(join_err) => Err(RuntimeError::Panic(join_err.to_string())),
        }
    }
}

fn generate_protocol_key_set() -> LocalApiKeySet {
    let openai = generate_local_api_key().into_string();
    let mut anthropic = generate_local_api_key().into_string();
    while anthropic == openai {
        anthropic = generate_local_api_key().into_string();
    }
    LocalApiKeySet::new(openai, anthropic)
}

fn local_api_config(addr: SocketAddr, keys: &LocalApiKeySet) -> LocalApiConfig {
    let root = format!("http://{addr}");
    LocalApiConfig {
        base_url: root.clone(),
        api_key: keys.openai.clone(),
        openai: LocalApiEndpointConfig {
            base_url: format!("{root}/v1"),
            api_key: keys.openai.clone(),
        },
        anthropic: LocalApiEndpointConfig {
            base_url: root,
            api_key: keys.anthropic.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a runtime that lets the OS allocate a port — tests must
    /// not race over fixed ports like 8787.
    fn ephemeral_runtime() -> ProxyRuntime {
        ProxyRuntime::new(0, 1)
    }

    #[test]
    fn with_dependencies_reuses_injected_handles() {
        let manager = Arc::new(ProviderManager::new());
        let engine: SwitchEngineHandle = Arc::new(SwitchEngine::new(Arc::clone(&manager)));
        let runtime =
            ProxyRuntime::with_dependencies(0, 1, Arc::clone(&engine), Arc::clone(&manager));

        assert!(Arc::ptr_eq(&runtime.state.engine, &engine));
        assert!(Arc::ptr_eq(&runtime.state.manager, &manager));
    }

    #[tokio::test]
    async fn current_config_is_none_before_start() {
        let runtime = ephemeral_runtime();
        assert!(runtime.current_config().await.is_none());
    }

    #[tokio::test]
    async fn start_returns_loopback_config_with_fresh_key() {
        let runtime = ephemeral_runtime();
        let config = runtime.start().await.expect("start should succeed");
        assert!(config.base_url.starts_with("http://127.0.0.1:"));
        assert!(config.api_key.starts_with("sk-local-"));
        assert_eq!(config.api_key, config.openai.api_key);
        assert_eq!(config.anthropic.base_url, config.base_url);
        assert_eq!(config.openai.base_url, format!("{}/v1", config.base_url));
        assert!(config.anthropic.api_key.starts_with("sk-local-"));
        assert_ne!(config.openai.api_key, config.anthropic.api_key);
        runtime.stop().await.expect("stop should succeed");
    }

    #[tokio::test]
    async fn double_start_is_rejected_with_already_running() {
        let runtime = ephemeral_runtime();
        runtime.start().await.expect("first start should succeed");
        let err = runtime
            .start()
            .await
            .expect_err("second start must surface AlreadyRunning");
        matches!(err, RuntimeError::AlreadyRunning(_));
        runtime.stop().await.expect("stop should succeed");
    }

    #[tokio::test]
    async fn regenerate_api_key_rotates_key_without_bouncing_server() {
        let runtime = ephemeral_runtime();
        let first = runtime.start().await.expect("start should succeed");
        let second = runtime
            .regenerate_api_key()
            .await
            .expect("regenerate should succeed");
        assert_eq!(
            first.base_url, second.base_url,
            "regenerate must not change port",
        );
        assert_ne!(
            first.api_key, second.api_key,
            "regenerate must rotate the OpenAI-compatible key",
        );
        assert_ne!(
            first.anthropic.api_key, second.anthropic.api_key,
            "regenerate must rotate the Anthropic key",
        );
        runtime.stop().await.expect("stop should succeed");
    }

    #[tokio::test]
    async fn regenerate_when_stopped_returns_not_running() {
        let runtime = ephemeral_runtime();
        let err = runtime
            .regenerate_api_key()
            .await
            .expect_err("must error when not running");
        matches!(err, RuntimeError::NotRunning);
    }

    #[tokio::test]
    async fn restart_rotates_key_and_returns_fresh_config() {
        let runtime = ephemeral_runtime();
        let before = runtime.start().await.expect("start should succeed");
        let after = runtime.restart().await.expect("restart should succeed");
        assert_ne!(
            before.api_key, after.api_key,
            "restart must produce a new OpenAI-compatible key",
        );
        assert_ne!(
            before.anthropic.api_key, after.anthropic.api_key,
            "restart must produce a new Anthropic key",
        );
        assert!(after.base_url.starts_with("http://127.0.0.1:"));
        assert_eq!(after.openai.base_url, format!("{}/v1", after.base_url));
        runtime.stop().await.expect("stop should succeed");
    }

    #[tokio::test]
    async fn stop_is_idempotent() {
        let runtime = ephemeral_runtime();
        runtime.start().await.expect("start should succeed");
        runtime.stop().await.expect("first stop should succeed");
        runtime
            .stop()
            .await
            .expect("second stop on idle runtime should be a no-op");
    }
}
