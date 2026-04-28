//! System tray integration (T1.0.4.15–16).
//!
//! Builds a native tray icon with a context menu:
//! status line, current provider, show/copy/restart/quit actions.

use std::sync::Arc;

use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager};

use crate::proxy::ProxyRuntime;

/// Menu-item IDs — stable strings so event matching is reliable.
const ID_SHOW_WINDOW: &str = "tray_show_window";
const ID_COPY_API_KEY: &str = "tray_copy_api_key";
const ID_RESTART_PROXY: &str = "tray_restart_proxy";
const ID_QUIT: &str = "tray_quit";

/// Build and register the system tray. Called from `lib.rs` setup.
///
/// # Errors
///
/// Returns a boxed error if the tray icon or menu cannot be created.
pub fn setup(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let show = MenuItemBuilder::with_id(ID_SHOW_WINDOW, "Show Window").build(app)?;
    let copy_key = MenuItemBuilder::with_id(ID_COPY_API_KEY, "Copy API Key").build(app)?;
    let restart = MenuItemBuilder::with_id(ID_RESTART_PROXY, "Restart Proxy").build(app)?;
    let quit = MenuItemBuilder::with_id(ID_QUIT, "Quit").build(app)?;

    let menu = MenuBuilder::new(app)
        .text("tray_status", "CCUse \u{2014} Running")
        .separator()
        .item(&show)
        .item(&copy_key)
        .item(&restart)
        .separator()
        .item(&quit)
        .build()?;

    let app_handle = app.clone();
    TrayIconBuilder::new()
        .icon(app.default_window_icon().cloned().expect("icon must exist"))
        .icon_as_template(true)
        .menu(&menu)
        .on_menu_event(move |_tray, event| {
            handle_menu_event(&app_handle, event.id().as_ref());
        })
        .build(app)?;

    Ok(())
}

fn handle_menu_event(app: &AppHandle, id: &str) {
    match id {
        ID_SHOW_WINDOW => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        ID_COPY_API_KEY => {
            let runtime = app.state::<Arc<ProxyRuntime>>();
            let rt = runtime.inner().clone();
            let app_handle = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Some(config) = rt.current_config().await {
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let js = format!(
                            "navigator.clipboard.writeText({0})",
                            serde_json::to_string(&config.api_key).unwrap_or_default()
                        );
                        let _ = window.eval(&js);
                    }
                }
            });
        }
        ID_RESTART_PROXY => {
            let runtime = app.state::<Arc<ProxyRuntime>>();
            let rt = runtime.inner().clone();
            let app_handle = app.clone();
            tauri::async_runtime::spawn(async move {
                match rt.restart().await {
                    Ok(config) => {
                        let _ = app_handle.emit(
                            crate::commands::proxy::EVENT_LOCAL_API_CONFIG_CHANGED,
                            &config,
                        );
                    }
                    Err(err) => {
                        eprintln!("CCUse tray: restart failed: {err}");
                    }
                }
            });
        }
        ID_QUIT => {
            std::process::exit(0);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_item_ids_are_stable() {
        assert_eq!(ID_SHOW_WINDOW, "tray_show_window");
        assert_eq!(ID_COPY_API_KEY, "tray_copy_api_key");
        assert_eq!(ID_RESTART_PROXY, "tray_restart_proxy");
        assert_eq!(ID_QUIT, "tray_quit");
    }
}
