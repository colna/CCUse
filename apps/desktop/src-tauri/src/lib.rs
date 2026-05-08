//! `CCUse` desktop runtime entry.
//!
//! `main.rs` delegates to [`run`] so the same entry point can be reused
//! by the future mobile target. The local proxy server, providers, and
//! switch engine will be wired in here in later phases.

pub mod auth;
pub mod commands;
pub mod config_export;
pub mod converter;
pub mod crypto;
pub mod db;
pub mod health;
pub mod panic_hook;
pub mod providers;
pub mod proxy;
pub mod switch;
pub mod tray;

use std::sync::Arc;

use tauri::Manager;

use commands::model_mapping::ModelMappingHandle;
use commands::providers::{HealthCheckerHandle, ProviderManagerHandle, ProviderRepoHandle};
use commands::switch::SwitchEngineHandle;
use providers::ProviderManager;
use proxy::ProxyRuntime;
use switch::SwitchEngine;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Shared provider manager for switch engine + health checker.
    let manager: ProviderManagerHandle = Arc::new(ProviderManager::new());
    let engine: SwitchEngineHandle = Arc::new(SwitchEngine::new(Arc::clone(&manager)));
    let checker: HealthCheckerHandle = Arc::new(health::HealthChecker::new(Arc::clone(&manager)));
    let model_mapping: ModelMappingHandle =
        Arc::new(tokio::sync::RwLock::new(converter::ModelMapping::new()));

    let runtime_engine = Arc::clone(&engine);
    let runtime_manager = Arc::clone(&manager);

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .manage(engine)
        .manage(Arc::clone(&checker))
        .manage(model_mapping)
        .manage(Arc::clone(&manager))
        .invoke_handler(tauri::generate_handler![
            // Proxy (T1.0.1)
            commands::proxy::get_local_api_config,
            commands::proxy::regenerate_api_key,
            commands::proxy::restart_proxy,
            // Provider CRUD (T1.0.2.19) + test connection (T1.0.4.05)
            commands::providers::list_providers,
            commands::providers::add_provider,
            commands::providers::update_provider,
            commands::providers::delete_provider,
            commands::providers::test_provider_connection,
            // Strategy (T1.0.2.20)
            commands::switch::get_strategy,
            commands::switch::set_strategy,
            commands::switch::update_strategy_params,
            // Health (T1.0.2.21)
            commands::health::get_health_snapshot,
            commands::health::refresh_health_snapshot,
            // Model mapping commands are retained for backward compatibility;
            // outbound provider requests ignore these mappings.
            commands::model_mapping::get_model_mappings,
            commands::model_mapping::set_model_mapping,
            commands::model_mapping::remove_model_mapping,
            // Monitor (T1.0.4.14)
            commands::monitor::get_metrics_timeseries,
            commands::monitor::get_switch_timeline,
            commands::monitor::get_provider_cost_summary,
            // Notification (T1.0.4.17)
            commands::notification::send_notification,
            // Config export / import / presets (T1.0.4.18–20)
            commands::config_export::export_config_json,
            commands::config_export::import_config_json,
            commands::config_export::get_template_presets,
        ])
        .setup(move |app| {
            // Initialise the database and provider repository.
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("app data dir must be resolvable");

            // T1.0.5.07: install panic hook before spawning any tasks.
            panic_hook::install_panic_hook(app_dir.clone());
            let db_path = app_dir.join("ccuse.db");
            let database = db::open_database(&db_path).expect("failed to open database");
            db::run_migrations(&database).expect("failed to run migrations");

            let key_store_path = app_dir.join(crypto::FILE_KEY_STORE_NAME);
            let backend = crypto::FileKeyringBackend::new(key_store_path);
            let master_key = Arc::new(
                crypto::load_or_create_master_key(&backend)
                    .expect("failed to initialise master key"),
            );
            app.manage(database.clone());
            let repo: ProviderRepoHandle = Arc::new(providers::ProviderRepository::new(
                database.clone(),
                master_key,
            ));
            app.manage(Arc::clone(&repo));

            let runtime: commands::RuntimeHandle =
                Arc::new(ProxyRuntime::with_dependencies_and_monitoring(
                    proxy::DEFAULT_PROXY_PORT,
                    proxy::DEFAULT_PROXY_ATTEMPTS,
                    Arc::clone(&runtime_engine),
                    Arc::clone(&runtime_manager),
                    database,
                ));
            app.manage(Arc::clone(&runtime));

            checker.forward_events_to_app(app.handle().clone());

            // T1.0.6.04: hydrate ProviderManager from the DB before
            // accepting any /v1/* traffic; failure is logged and the
            // manager stays empty so /v1/* returns 503 instead of crashing.
            // T1.0.6.28 then starts the health loop so status changes
            // are pushed to the UI via provider-status-changed.
            let load_manager = Arc::clone(&manager);
            let load_repo = Arc::clone(&repo);
            let load_checker = Arc::clone(&checker);
            tauri::async_runtime::spawn(async move {
                providers::load_initial_providers(&load_manager, &load_repo).await;
                load_checker.probe_once().await;
                load_checker.start(health::DEFAULT_CHECK_INTERVAL).await;
            });

            // Boot the proxy on startup so the UI can read the config
            // immediately. Errors here are non-fatal; the UI surfaces
            // "not running" through `get_local_api_config`.
            tauri::async_runtime::spawn(async move {
                if let Err(err) = runtime.start().await {
                    eprintln!("CCUse: proxy failed to start: {err}");
                }
            });

            // System tray (T1.0.4.15–16).
            if let Err(err) = tray::setup(app.handle()) {
                eprintln!("CCUse: tray setup failed: {err}");
            }

            Ok(())
        });

    // T1.0.4.24: Intercept close → hide to tray instead of quitting.
    let builder = builder.on_window_event(|window, event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = window.hide();
        }
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
