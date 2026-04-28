//! `Tauri` command surface for the local proxy.
//!
//! Thin wrappers over [`ProxyRuntime`]; the real logic — and the unit
//! tests — live in `proxy::runtime`. Errors are stringified at this
//! boundary because Tauri's IPC bridge serialises everything as JSON.

use std::sync::Arc;

use tauri::State;

use crate::proxy::{LocalApiConfig, ProxyRuntime};

/// Type alias for the managed runtime handle. Always `Arc` so we
/// can share one instance across commands without cloning the inner
/// `Mutex`.
pub type RuntimeHandle = Arc<ProxyRuntime>;

/// `get_local_api_config` — UI binds this to the "Local API Service"
/// card (T1.0.1.24). Returns `None` while the proxy is bouncing.
#[tauri::command]
pub async fn get_local_api_config(
    state: State<'_, RuntimeHandle>,
) -> Result<LocalApiConfig, String> {
    state
        .current_config()
        .await
        .ok_or_else(|| "proxy is not running".to_owned())
}

/// `regenerate_api_key` — UI binds this to the "Rotate" button in the
/// service card. Returns the freshly issued config.
#[tauri::command]
pub async fn regenerate_api_key(state: State<'_, RuntimeHandle>) -> Result<LocalApiConfig, String> {
    state.regenerate_api_key().await.map_err(|e| e.to_string())
}

/// `restart_proxy` — UI binds this to the "Restart" button. Bounces
/// the listener (port may change) and rotates the key.
#[tauri::command]
pub async fn restart_proxy(state: State<'_, RuntimeHandle>) -> Result<LocalApiConfig, String> {
    state.restart().await.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `current_config` is `None` before `start` — the command layer
    /// translates that into `Err("proxy is not running")` so the
    /// frontend can branch on `result.error`.
    #[tokio::test]
    async fn get_config_before_start_returns_string_error() {
        let runtime = Arc::new(ProxyRuntime::new(0, 1));
        let result = runtime.current_config().await;
        assert!(result.is_none());
        let err = result
            .ok_or_else(|| "proxy is not running".to_owned())
            .expect_err("ok_or_else should hit error path");
        assert_eq!(err, "proxy is not running");
    }

    #[tokio::test]
    async fn regenerate_via_runtime_returns_new_key() {
        let runtime = Arc::new(ProxyRuntime::new(0, 1));
        let first = runtime.start().await.expect("start ok");
        let second = runtime.regenerate_api_key().await.expect("regen ok");
        assert_ne!(first.api_key, second.api_key);
        runtime.stop().await.expect("stop ok");
    }

    #[tokio::test]
    async fn restart_via_runtime_rotates_key() {
        let runtime = Arc::new(ProxyRuntime::new(0, 1));
        let before = runtime.start().await.expect("start ok");
        let after = runtime.restart().await.expect("restart ok");
        assert_ne!(before.api_key, after.api_key);
        runtime.stop().await.expect("stop ok");
    }
}
