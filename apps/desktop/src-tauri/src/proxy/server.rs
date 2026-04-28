//! `axum`-based HTTP server scaffolding for the local proxy.
//!
//! Phase 1.0.1 scope: only a `/healthz` endpoint and a clean
//! graceful-shutdown contract. Real routes (`/v1/chat/completions`
//! etc.) are introduced in T1.0.1.08; provider dispatch in T1.0.2.

use std::future::Future;
use std::net::SocketAddr;

use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::{get, post};
use axum::Router;
use serde_json::{json, Value};
use tokio::net::TcpListener;

/// Errors raised while binding or running the proxy server.
#[derive(thiserror::Error, Debug)]
pub enum ServerError {
    /// Failed to bind a `TcpListener` to the requested address.
    #[error("failed to bind tcp listener at {addr}: {source}")]
    Bind {
        addr: SocketAddr,
        #[source]
        source: std::io::Error,
    },

    /// Probed `attempts` consecutive ports from `start` and none was available.
    #[error(
        "no available port in range [{start}, {}); last attempt: {last:?}",
        start.saturating_add(*attempts)
    )]
    NoAvailablePort {
        start: u16,
        attempts: u16,
        #[source]
        last: Option<Box<ServerError>>,
    },

    /// `start + offset` would overflow `u16` while probing.
    #[error("port probe overflowed u16 starting at {start} after {offset} steps")]
    PortOverflow { start: u16, offset: u16 },

    /// `axum::serve` returned an io error while running.
    #[error("axum serve loop exited with error: {0}")]
    Serve(#[from] std::io::Error),
}

/// A bound proxy server, ready to accept connections.
///
/// Two-step lifecycle so callers can read [`ProxyServer::local_addr`]
/// before they hand the server off to a long-lived task. This is what
/// the port-prober (T1.0.1.07) and the tray UI (T1.0.4.15) need to
/// surface "listening on port N" to the user.
pub struct ProxyServer {
    listener: TcpListener,
    local_addr: SocketAddr,
}

impl std::fmt::Debug for ProxyServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // listener intentionally omitted — its inner state is not useful
        // in logs and `TcpListener`'s Debug output is verbose.
        f.debug_struct("ProxyServer")
            .field("local_addr", &self.local_addr)
            .finish_non_exhaustive()
    }
}

impl ProxyServer {
    /// Bind to `addr`. Use port `0` to let the OS pick — the actual
    /// port is then available via [`ProxyServer::local_addr`].
    pub async fn bind(addr: SocketAddr) -> Result<Self, ServerError> {
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|source| ServerError::Bind { addr, source })?;
        let local_addr = listener
            .local_addr()
            .map_err(|source| ServerError::Bind { addr, source })?;
        Ok(Self {
            listener,
            local_addr,
        })
    }

    /// Probe ports `[start, start + attempts)` on `127.0.0.1` and bind
    /// to the first available one.
    ///
    /// Drives the desktop default flow: try `8787`, walk up to `8886`
    /// if `8787` is already taken. The bound port is reported via
    /// [`ProxyServer::local_addr`] so the tray / UI can surface it.
    pub async fn bind_with_fallback(start: u16, attempts: u16) -> Result<Self, ServerError> {
        let mut last: Option<ServerError> = None;
        for offset in 0..attempts {
            let port = start
                .checked_add(offset)
                .ok_or(ServerError::PortOverflow { start, offset })?;
            let addr: SocketAddr = ([127, 0, 0, 1], port).into();
            match Self::bind(addr).await {
                Ok(server) => return Ok(server),
                Err(err) => last = Some(err),
            }
        }
        Err(ServerError::NoAvailablePort {
            start,
            attempts,
            last: last.map(Box::new),
        })
    }

    /// The address actually bound. Differs from the `addr` passed to
    /// [`ProxyServer::bind`] when port `0` was requested.
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Run the server until `shutdown` resolves.
    ///
    /// Static dispatch on the shutdown future (per `rust-best-practices`
    /// §6) — callers usually pass `tokio::signal::ctrl_c()` or a
    /// `oneshot::Receiver`; both are zero-cost here.
    pub async fn serve_with_shutdown<F>(self, shutdown: F) -> Result<(), ServerError>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let app = build_router();
        axum::serve(self.listener, app)
            .with_graceful_shutdown(shutdown)
            .await?;
        Ok(())
    }
}

/// Build the router for the local proxy.
///
/// `/healthz` is the liveness probe used by tray + health-check loop.
/// The three `v1/*` routes are the unified API surface clients call
/// into; their handlers are stubs until T1.0.2 wires the provider
/// dispatch — they return 503 with an `OpenAI`-shaped error body.
fn build_router() -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/models", get(list_models))
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/messages", post(anthropic_messages))
}

/// `GET /healthz` — returns `200 ok`. Minimal payload on purpose;
/// the rich health snapshot lives in `T1.0.2.21 get_health_snapshot`
/// (a Tauri command, not an HTTP route).
async fn healthz() -> &'static str {
    "ok"
}

/// `GET /v1/models` — returns an empty list until the provider
/// registry (T1.0.2.03 `ProviderManager`) is wired.
///
/// Shape mirrors `OpenAI`'s `/v1/models` so generic clients see
/// "no models available" instead of a hard 404.
async fn list_models() -> Json<Value> {
    Json(json!({
        "object": "list",
        "data": [],
    }))
}

/// `POST /v1/chat/completions` — `OpenAI`-format inbound. Stub until
/// T1.0.2.15 `SwitchEngine` `execute_request` is plumbed in.
async fn chat_completions() -> (StatusCode, Json<Value>) {
    not_configured_response()
}

/// `POST /v1/messages` — Anthropic-format inbound. Stub until
/// T1.0.3.04 + T1.0.2.15 land.
async fn anthropic_messages() -> (StatusCode, Json<Value>) {
    not_configured_response()
}

/// Standard 503 body returned by stub handlers. Shape mirrors
/// `OpenAI`'s error envelope so clients that already parse it work
/// unchanged once real dispatch lands.
fn not_configured_response() -> (StatusCode, Json<Value>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({
            "error": {
                "type": "providers_not_configured",
                "message": "Provider dispatch is not wired yet. \
                            Configure providers in CCUse settings \
                            (this is a Phase 1.0.1 stub).",
            }
        })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_error_bind_renders_address_and_source() {
        let err = ServerError::Bind {
            addr: ([127, 0, 0, 1], 65535).into(),
            source: std::io::Error::new(std::io::ErrorKind::AddrInUse, "boom"),
        };
        let rendered = format!("{err}");
        assert!(rendered.contains("127.0.0.1:65535"));
        assert!(rendered.contains("boom"));
    }

    #[test]
    fn server_error_no_available_port_renders_range() {
        let err = ServerError::NoAvailablePort {
            start: 8787,
            attempts: 100,
            last: None,
        };
        let rendered = format!("{err}");
        assert!(rendered.contains("8787"));
        assert!(rendered.contains("8887"));
    }

    #[test]
    fn server_error_port_overflow_renders_start_and_offset() {
        let err = ServerError::PortOverflow {
            start: u16::MAX - 2,
            offset: 5,
        };
        let rendered = format!("{err}");
        assert!(rendered.contains(&(u16::MAX - 2).to_string()));
        assert!(rendered.contains('5'));
    }
}
