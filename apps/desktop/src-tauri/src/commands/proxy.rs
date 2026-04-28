//! `Tauri` command surface for the local proxy.
//!
//! Thin wrappers over [`ProxyRuntime`]; the real logic — and the unit
//! tests — live in `proxy::runtime`. Errors are stringified at this
//! boundary because Tauri's IPC bridge serialises everything as JSON.
//!
//! On every successful mutation we emit
//! [`EVENT_LOCAL_API_CONFIG_CHANGED`] so multi-window UIs (T1.0.1.26)
//! and the future tray (T1.0.4.15) can refresh in lockstep without
//! polling.

use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};

use crate::proxy::{LocalApiConfig, ProxyRuntime};

/// Type alias for the managed runtime handle. Always `Arc` so we
/// can share one instance across commands without cloning the inner
/// `Mutex`.
pub type RuntimeHandle = Arc<ProxyRuntime>;

/// Event name emitted whenever the proxy's `LocalApiConfig` changes.
/// UI uses `@tauri-apps/api/event#listen` to react.
pub const EVENT_LOCAL_API_CONFIG_CHANGED: &str = "local_api_config_changed";

fn broadcast_config_change(app: &AppHandle, payload: &LocalApiConfig) {
    // Best-effort: a missing window or unmounted listener is not a
    // hard failure — drop the error rather than break the command.
    let _ = app.emit(EVENT_LOCAL_API_CONFIG_CHANGED, payload);
}

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
/// service card. Returns the freshly issued config and broadcasts
/// `local_api_config_changed`.
#[tauri::command]
pub async fn regenerate_api_key(
    app: AppHandle,
    state: State<'_, RuntimeHandle>,
) -> Result<LocalApiConfig, String> {
    let config = state
        .regenerate_api_key()
        .await
        .map_err(|e| e.to_string())?;
    broadcast_config_change(&app, &config);
    Ok(config)
}

/// `restart_proxy` — UI binds this to the "Restart" button. Bounces
/// the listener (port may change), rotates the key, and broadcasts
/// `local_api_config_changed`.
#[tauri::command]
pub async fn restart_proxy(
    app: AppHandle,
    state: State<'_, RuntimeHandle>,
) -> Result<LocalApiConfig, String> {
    let config = state.restart().await.map_err(|e| e.to_string())?;
    broadcast_config_change(&app, &config);
    Ok(config)
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

    #[test]
    fn event_name_is_stable_wire_id() {
        // Frontend `lib/tauri.ts#onLocalApiConfigChanged` listens on
        // this exact string. Renaming silently would break every UI
        // subscriber, so the test pins it.
        assert_eq!(EVENT_LOCAL_API_CONFIG_CHANGED, "local_api_config_changed");
    }
}
