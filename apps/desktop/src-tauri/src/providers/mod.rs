//! Provider domain types and persistence.
//!
//! Phase 1.0.1 ships:
//! * [`Provider`] / [`ProviderInput`] / [`ProviderKind`] domain types
//! * [`repository`] — CRUD over `providers` table with at-rest
//!   encryption of API keys
//!
//! T1.0.1.19 adds the runtime [`Provider`] trait (HTTP dispatch); the
//! domain [`Provider`] here is the persistence shape, not the trait.

pub mod api;
pub mod manager;
pub mod model;
pub mod openai;
pub mod repository;
pub mod startup;
pub mod wrapper;

pub use api::{
    ApiChoice, ApiRequest, ApiResponse, ApiToolDefinition, ApiUsage, ChatMessage, HealthStatus,
    Provider as RuntimeProvider, ProviderError, StreamChunk, StreamingResponse,
};
pub use manager::{ManagerError, ProviderManager};
pub use model::{Provider, ProviderInput, ProviderKind};
pub use openai::{OpenAIProvider, DEFAULT_REQUEST_TIMEOUT};
pub use repository::{ProviderRepository, RepositoryError};
pub use startup::load_initial_providers;
pub use wrapper::{ProviderWrapper, RuntimeState};
