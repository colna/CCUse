//! Local HTTP proxy server.
//!
//! Owns the `axum` runtime that exposes the unified API surface
//! (`/v1/chat/completions`, `/v1/messages`, …) clients call into.
//! Wired into the `Tauri` lifecycle in later phases (T1.0.1.12).

pub mod error;
pub mod server;

pub use error::{ApiError, ApiErrorKind};
pub use server::{ProxyServer, ServerError};
