//! T1.0.2.09–16 — Provider switching strategies and retry engine.
//!
//! * [`SwitchStrategy`] enum with five selection algorithms
//! * [`SwitchEngine`] — retry chain with automatic failover
//! * Error classification via [`ProviderError::is_retriable`]

pub mod engine;
pub mod strategy;

pub use engine::{DispatchResult, SwitchConfig, SwitchEngine, DEFAULT_MAX_RETRIES};
pub use strategy::{RoundRobinState, SmartWeights, SwitchStrategy};
