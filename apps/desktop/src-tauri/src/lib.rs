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
pub mod providers;
pub mod proxy;
pub mod switch;
pub mod tray;

use std::sync::Arc;

use tauri::Manager;

use commands::health::HealthCheckerHandle;
use commands::model_mapping::ModelMappingHandle;
use commands::providers::ProviderRepoHandle;
use commands::switch::SwitchEngineHandle;
use providers::ProviderManager;
use proxy::ProxyRuntime;
use switch::SwitchEngine;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let runtime: commands::RuntimeHandle = Arc::new(ProxyRuntime::default());
    let startup = runtime.clone();

    // Shared provider manager for switch engine + health checker.
    let manager = Arc::new(ProviderManager::new());
    let engine: SwitchEngineHandle = Arc::new(SwitchEngine::new(Arc::clone(&manager)));
    let checker: HealthCheckerHandle = Arc::new(health::HealthChecker::new(Arc::clone(&manager)));

    // Model mapping with defaults (T1.0.3.12).
    let model_mapping: ModelMappingHandle =
        Arc::new(tokio::sync::RwLock::new(converter::ModelMapping::new()));

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .manage(runtime)
        .manage(engine)
        .manage(checker)
        .manage(model_mapping)
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
            // Model mapping (T1.0.3.12)
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
            let db_path = app_dir.join("ccuse.db");
            let database = db::open_database(&db_path).expect("failed to open database");
            db::run_migrations(&database).expect("failed to run migrations");

            let master_key = Arc::new(
                crypto::load_or_create_master_key(&crypto::master_key::OsKeyringBackend)
                    .expect("failed to initialise master key"),
            );
            app.manage(database.clone());
            let repo: ProviderRepoHandle =
                Arc::new(providers::ProviderRepository::new(database, master_key));
            app.manage(repo);

            // Boot the proxy on startup so the UI can read the config
            // immediately. Errors here are non-fatal; the UI surfaces
            // "not running" through `get_local_api_config`.
            let runtime = startup.clone();
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
