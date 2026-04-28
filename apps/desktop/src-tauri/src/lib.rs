//! `CCUse` desktop runtime entry.
//!
//! `main.rs` delegates to [`run`] so the same entry point can be reused
//! by the future mobile target. The local proxy server, providers, and
//! switch engine will be wired in here in later phases.

pub mod auth;
pub mod commands;
pub mod crypto;
pub mod db;
pub mod health;
pub mod providers;
pub mod proxy;
pub mod switch;

use std::sync::Arc;

use tauri::Manager;

use commands::health::HealthCheckerHandle;
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

    let builder = tauri::Builder::default()
        .manage(runtime)
        .manage(engine)
        .manage(checker)
        .invoke_handler(tauri::generate_handler![
            // Proxy (T1.0.1)
            commands::proxy::get_local_api_config,
            commands::proxy::regenerate_api_key,
            commands::proxy::restart_proxy,
            // Provider CRUD (T1.0.2.19)
            commands::providers::list_providers,
            commands::providers::add_provider,
            commands::providers::update_provider,
            commands::providers::delete_provider,
            // Strategy (T1.0.2.20)
            commands::switch::get_strategy,
            commands::switch::set_strategy,
            commands::switch::update_strategy_params,
            // Health (T1.0.2.21)
            commands::health::get_health_snapshot,
        ])
        .setup(move |app| {
            // Initialise the database and provider repository.
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("app data dir must be resolvable");
            let db_path = app_dir.join("ccuse.db");
            let database = db::open_database(&db_path)
                .expect("failed to open database");
            db::run_migrations(&database).expect("failed to run migrations");

            let master_key = Arc::new(
                crypto::load_or_create_master_key(&crypto::master_key::OsKeyringBackend)
                    .expect("failed to initialise master key"),
            );
            let repo: ProviderRepoHandle = Arc::new(
                providers::ProviderRepository::new(database, master_key),
            );
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
