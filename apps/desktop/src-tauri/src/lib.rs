//! `CCUse` desktop runtime entry.
//!
//! `main.rs` delegates to [`run`] so the same entry point can be reused
//! by the future mobile target. The local proxy server, providers, and
//! switch engine will be wired in here in later phases.

pub mod auth;
pub mod commands;
pub mod crypto;
pub mod db;
pub mod providers;
pub mod proxy;

use std::sync::Arc;

use proxy::ProxyRuntime;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let runtime: commands::RuntimeHandle = Arc::new(ProxyRuntime::default());
    let startup = runtime.clone();

    let builder = tauri::Builder::default()
        .manage(runtime)
        .invoke_handler(tauri::generate_handler![
            commands::get_local_api_config,
            commands::regenerate_api_key,
            commands::restart_proxy,
        ])
        .setup(move |_app| {
            // Boot the proxy on startup so the UI can read the config
            // immediately. Errors here are non-fatal; the UI surfaces
            // "not running" through `get_local_api_config`.
            let runtime = startup.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(err) = runtime.start().await {
                    eprintln!("CCUse: proxy failed to start: {err}");
                }
            });
            Ok(())
        });

    if let Err(err) = builder.run(tauri::generate_context!()) {
        // Surface the cause to stderr; exit non-zero so the OS / launcher
        // can detect that startup failed instead of swallowing the error.
        eprintln!("CCUse failed to start: {err}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    /// Smoke test: keeps the lib crate test target alive so future
    /// modules can drop in `#[test]` items without scaffolding noise.
    #[test]
    fn lib_crate_smoke_test_runs() {
        assert_eq!(2 + 2, 4);
    }
}
