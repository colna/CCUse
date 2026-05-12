//! Local HTTP proxy server.
//!
//! Owns the `axum` runtime that exposes the unified API surface
//! (`/v1/chat/completions`, `/v1/messages`, …) clients call into.
//! Wired into the `Tauri` lifecycle in later phases (T1.0.1.12).

pub mod bridge;
pub mod error;
pub mod runtime;
pub mod server;
pub mod sse;

pub use error::{ApiError, ApiErrorKind};
pub use runtime::{
    LocalApiConfig, LocalApiEndpointConfig, ProxyRuntime, RuntimeError, DEFAULT_PROXY_ATTEMPTS,
    DEFAULT_PROXY_PORT,
};
pub use server::{ProxyAppState, ProxyServer, ServerError};
