//! Tauri command surface for `CCUse`.
//!
//! Sub-modules group commands by domain:
//! * [`proxy`] — local proxy lifecycle (T1.0.1)
//! * [`providers`] — provider CRUD (T1.0.2.19)
//! * [`switch`] — strategy read/write (T1.0.2.20)
//! * [`health`] — health snapshot (T1.0.2.21)

pub mod health;
pub mod providers;
pub mod proxy;
pub mod switch;

// Re-export everything the `generate_handler!` macro in lib.rs needs.
pub use health::get_health_snapshot;
pub use providers::{add_provider, delete_provider, list_providers, update_provider};
pub use proxy::{
    get_local_api_config, regenerate_api_key, restart_proxy, RuntimeHandle,
    EVENT_LOCAL_API_CONFIG_CHANGED,
};
pub use switch::{get_strategy, set_strategy, update_strategy_params};
