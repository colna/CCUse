//! `axum`-based HTTP server scaffolding for the local proxy.
//!
//! Phase 1.0.1 scope: only a `/healthz` endpoint and a clean
//! graceful-shutdown contract. Real routes (`/v1/chat/completions`
//! etc.) are introduced in T1.0.1.08; provider dispatch in T1.0.2.

use std::future::Future;
use std::net::SocketAddr;

use axum::routing::get;
use axum::Router;
use tokio::net::TcpListener;

/// Errors raised while binding or running the proxy server.
///
/// Kept narrow on purpose — the server has only two failure modes:
/// it could not bind, or `axum::serve` exited with an io error.
#[derive(thiserror::Error, Debug)]
pub enum ServerError {
    /// Failed to bind a `TcpListener` to the requested address.
    #[error("failed to bind tcp listener at {addr}: {source}")]
    Bind {
        addr: SocketAddr,
        #[source]
        source: std::io::Error,
    },

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
/// Right now: only the liveness probe used by the tray indicator
/// and the health-check pipeline. Real provider routes are added in
/// T1.0.1.08 and after.
fn build_router() -> Router {
    Router::new().route("/healthz", get(healthz))
}

/// `GET /healthz` — returns `200 ok`. Minimal payload on purpose;
/// the rich health snapshot lives in `T1.0.2.21 get_health_snapshot`
/// (a Tauri command, not an HTTP route).
async fn healthz() -> &'static str {
    "ok"
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
}
