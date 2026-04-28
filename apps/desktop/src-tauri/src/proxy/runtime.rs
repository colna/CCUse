//! Runtime supervisor for the local proxy.
//!
//! Holds the live [`ProxyServer`] handle, the issued [`LocalApiKey`],
//! and the shutdown channel — i.e. everything T1.0.1.12 commands need
//! to mutate. Pure-Rust API so tests don't have to spin up `Tauri`.

use std::net::SocketAddr;

use serde::Serialize;
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

use crate::auth::{generate_local_api_key, key_store, KeyStore, LocalApiKey};

use super::server::{ProxyServer, ServerError};

/// Default loopback start port for the proxy. Probed range is
/// `[8787, 8887)` (100 attempts) — matches the desktop spec in
/// `docs/产品技术文档.md` §运行时配置.
pub const DEFAULT_PROXY_PORT: u16 = 8787;
/// Probe budget when [`DEFAULT_PROXY_PORT`] is busy.
pub const DEFAULT_PROXY_ATTEMPTS: u16 = 100;

/// Snapshot returned to the UI / clipboard. Cheap to clone (two
/// owned strings) — UI re-renders should not dictate this struct.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LocalApiConfig {
    /// Loopback HTTP base URL, e.g. `http://127.0.0.1:8787`.
    pub base_url: String,
    /// `sk-local-{32}` token issued at start / regenerate.
    pub api_key: String,
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
    api_key: LocalApiKey,
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
            .field("api_key", &"<redacted>")
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
        Self {
            inner: Mutex::new(None),
            fallback_start,
            fallback_attempts,
        }
    }

    /// Start the proxy if it's not already running. Generates a fresh
    /// `sk-local-…` key and returns the resulting config snapshot.
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
        let api_key = generate_local_api_key();
        let store = key_store(api_key.as_str().to_owned());
        let (shutdown, rx) = oneshot::channel::<()>();
        let handle = tokio::spawn(
            server.serve_with_auth_and_shutdown(store.clone(), async move {
                let _ = rx.await;
            }),
        );
        let config = LocalApiConfig {
            base_url: format!("http://{addr}"),
            api_key: api_key.as_str().to_owned(),
        };
        *guard = Some(RunningState {
            addr,
            api_key,
            key_store: store,
            shutdown,
            handle,
        });
        Ok(config)
    }

    /// Snapshot the current config, or `None` if the proxy isn't up.
    pub async fn current_config(&self) -> Option<LocalApiConfig> {
        let guard = self.inner.lock().await;
        guard.as_ref().map(|state| LocalApiConfig {
            base_url: format!("http://{}", state.addr),
            api_key: state.api_key.as_str().to_owned(),
        })
    }

    /// Replace the in-memory key with a freshly generated one. Does
    /// not bounce the server — existing connections keep their
    /// (already-authorised) request handles. `T1.0.1.13` middleware
    /// reads the current key per-request, so the swap takes effect
    /// on the next inbound call.
    pub async fn regenerate_api_key(&self) -> Result<LocalApiConfig, RuntimeError> {
        let mut guard = self.inner.lock().await;
        let state = guard.as_mut().ok_or(RuntimeError::NotRunning)?;
        let fresh = generate_local_api_key();
        // Push the new key into the auth keystore *before* swapping
        // the in-runtime copy: a request that races with rotation
        // either sees the old expected key (still valid) or the new
        // one — never an empty / inconsistent state.
        {
            let mut guard = state
                .key_store
                .write()
                .map_err(|_| RuntimeError::Serve("auth keystore lock poisoned".into()))?;
            fresh.as_str().clone_into(&mut guard);
        }
        state.api_key = fresh;
        Ok(LocalApiConfig {
            base_url: format!("http://{}", state.addr),
            api_key: state.api_key.as_str().to_owned(),
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a runtime that lets the OS allocate a port — tests must
    /// not race over fixed ports like 8787.
    fn ephemeral_runtime() -> ProxyRuntime {
        ProxyRuntime::new(0, 1)
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
            "regenerate must rotate the key",
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
            "restart must produce a new key",
        );
        assert!(after.base_url.starts_with("http://127.0.0.1:"));
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
