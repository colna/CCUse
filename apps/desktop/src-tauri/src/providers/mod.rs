//! Provider domain types and persistence.
//!
//! Phase 1.0.1 ships:
//! * [`Provider`] / [`ProviderInput`] / [`ProviderKind`] domain types
//! * [`repository`] — CRUD over `providers` table with at-rest
//!   encryption of API keys
//!
//! T1.0.1.19 adds the runtime [`Provider`] trait (HTTP dispatch); the
//! domain [`Provider`] here is the persistence shape, not the trait.

pub mod anthropic;
mod anthropic_headers;
pub mod api;
pub mod default_models;
mod error_format;
pub mod gemini;
pub mod manager;
pub mod model;
pub mod openai;
pub mod repository;
pub mod startup;
pub mod stream_check;
pub mod wrapper;

pub use anthropic::AnthropicProvider;
pub use api::{
    ApiChoice, ApiModel, ApiRequest, ApiResponse, ApiToolCall, ApiToolCallFunction,
    ApiToolDefinition, ApiUsage, ChatContent, ChatContentPart, ChatImageUrl, ChatMessage,
    HealthStatus, Provider as RuntimeProvider, ProviderError, StreamChunk, StreamingResponse,
};
pub use gemini::GeminiProvider;
pub use manager::{ManagerError, ProviderManager};
pub use model::{Provider, ProviderInput, ProviderKind};
pub use openai::{OpenAIProvider, DEFAULT_REQUEST_TIMEOUT};
pub use repository::{ProviderRepository, RepositoryError};
pub use startup::load_initial_providers;
pub use stream_check::{
    check_provider_with_default_config, StreamCheckConfig, StreamCheckResult, StreamCheckService,
    StreamCheckStatus, DEFAULT_DEGRADED_THRESHOLD_MS, DEFAULT_STREAM_CHECK_MAX_RETRIES,
    DEFAULT_STREAM_CHECK_TIMEOUT_SECS,
};
pub use wrapper::{ProviderWrapper, RuntimeState};
