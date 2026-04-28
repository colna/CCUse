//! Tauri command surface for `CCUse`.
//!
//! Sub-modules group commands by domain:
//! * [`proxy`] — local proxy lifecycle (T1.0.1)
//! * [`providers`] — provider CRUD (T1.0.2.19)
//! * [`switch`] — strategy read/write (T1.0.2.20)
//! * [`health`] — health snapshot (T1.0.2.21)
//! * [`model_mapping`] — model mapping CRUD (T1.0.3.12)
//! * [`monitor`] — aggregation queries + switch timeline (T1.0.4.14)

pub mod health;
pub mod model_mapping;
pub mod monitor;
pub mod providers;
pub mod proxy;
pub mod switch;

// Re-export everything the `generate_handler!` macro in lib.rs needs.
pub use health::get_health_snapshot;
pub use model_mapping::{get_model_mappings, remove_model_mapping, set_model_mapping};
pub use monitor::{get_metrics_timeseries, get_provider_cost_summary, get_switch_timeline};
pub use providers::{
    add_provider, delete_provider, list_providers, test_provider_connection, update_provider,
};
pub use proxy::{
    get_local_api_config, regenerate_api_key, restart_proxy, RuntimeHandle,
    EVENT_LOCAL_API_CONFIG_CHANGED,
};
pub use switch::{get_strategy, set_strategy, update_strategy_params};
