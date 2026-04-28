//! Integration tests for the local proxy server scaffold.
//!
//! Pin the three behaviors clients depend on:
//! 1. binding port `0` exposes the OS-assigned port via `local_addr`,
//! 2. `/healthz` responds `200 ok` once the server is running,
//! 3. resolving the shutdown future causes `serve` to return cleanly.

use std::net::SocketAddr;
use std::time::Duration;

use ccuse_desktop_lib::proxy::ProxyServer;
use tokio::sync::oneshot;

fn loopback_zero() -> SocketAddr {
    "127.0.0.1:0"
        .parse()
        .expect("loopback string is a valid SocketAddr")
}

#[tokio::test]
async fn bind_to_port_zero_resolves_real_loopback_port() {
    let server = ProxyServer::bind(loopback_zero())
        .await
        .expect("bind to 127.0.0.1:0 should succeed on a healthy host");

    let addr = server.local_addr();
    assert!(addr.ip().is_loopback(), "should bind to loopback only");
    assert_ne!(addr.port(), 0, "OS must replace 0 with a real port");
}

#[tokio::test]
async fn healthz_endpoint_responds_with_ok() {
    let server = ProxyServer::bind(loopback_zero())
        .await
        .expect("bind succeeds");
    let base = format!("http://{}", server.local_addr());
    let (tx, rx) = oneshot::channel::<()>();

    let serve_handle = tokio::spawn(server.serve_with_shutdown(async move {
        let _ = rx.await;
    }));

    // Brief yield so the server has a chance to start accepting.
    tokio::time::sleep(Duration::from_millis(50)).await;

    let response = reqwest::get(format!("{base}/healthz"))
        .await
        .expect("healthz request should reach the server");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let body = response.text().await.expect("body decodes as utf-8");
    assert_eq!(body, "ok");

    let _ = tx.send(());
    let serve_result = tokio::time::timeout(Duration::from_secs(2), serve_handle)
        .await
        .expect("server should shut down within 2s")
        .expect("join handle should not panic");
    assert!(
        serve_result.is_ok(),
        "serve should return Ok after shutdown"
    );
}

#[tokio::test]
async fn serve_returns_after_shutdown_signal() {
    let server = ProxyServer::bind(loopback_zero())
        .await
        .expect("bind succeeds");
    let (tx, rx) = oneshot::channel::<()>();

    let serve_handle = tokio::spawn(server.serve_with_shutdown(async move {
        let _ = rx.await;
    }));

    // Server is running. Fire shutdown immediately.
    tx.send(()).expect("shutdown receiver should still exist");

    let result = tokio::time::timeout(Duration::from_secs(2), serve_handle)
        .await
        .expect("graceful shutdown must complete within 2s");
    assert!(result.is_ok(), "serve task should not panic");
    assert!(
        result.expect("join ok").is_ok(),
        "serve should exit Ok after shutdown",
    );
}
