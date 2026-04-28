//! Provider domain types and persistence.
//!
//! Phase 1.0.1 ships:
//! * [`Provider`] / [`ProviderInput`] / [`ProviderKind`] domain types
//! * [`repository`] — CRUD over `providers` table with at-rest
//!   encryption of API keys
//!
//! T1.0.1.19 adds the runtime [`Provider`] trait (HTTP dispatch); the
//! domain [`Provider`] here is the persistence shape, not the trait.

pub mod model;
pub mod repository;

pub use model::{Provider, ProviderInput, ProviderKind};
pub use repository::{ProviderRepository, RepositoryError};
