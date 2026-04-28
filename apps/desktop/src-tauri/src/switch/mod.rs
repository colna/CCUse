//! T1.0.2.09–18 — Provider switching strategies, retry engine, and logging.
//!
//! * [`SwitchStrategy`] enum with five selection algorithms
//! * [`SwitchEngine`] — retry chain with automatic failover
//! * Error classification via [`ProviderError::is_retriable`]
//! * [`SwitchHistoryRepository`] — audit log for switch events (T1.0.2.17)
//! * [`RequestLogRepository`] — metadata log for proxied requests (T1.0.2.18)

pub mod engine;
pub mod history;
pub mod request_log;
pub mod strategy;

pub use engine::{DispatchResult, SwitchConfig, SwitchEngine, DEFAULT_MAX_RETRIES};
pub use history::{SwitchHistoryEntry, SwitchHistoryInput, SwitchHistoryRepository};
pub use request_log::{RequestLogEntry, RequestLogInput, RequestLogRepository};
pub use strategy::{RoundRobinState, SmartWeights, SwitchStrategy};
