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

#[tokio::test]
async fn bind_with_fallback_succeeds_with_single_attempt_on_zero() {
    // start=0 lets the OS allocate; one attempt always succeeds on a healthy host.
    let server = ProxyServer::bind_with_fallback(0, 1)
        .await
        .expect("OS should hand out an ephemeral port for start=0");
    assert!(server.local_addr().ip().is_loopback());
    assert_ne!(server.local_addr().port(), 0);
}

#[tokio::test]
async fn bind_with_fallback_skips_busy_port_and_finds_next() {
    // Hold one loopback port for the duration of the test, then ask
    // bind_with_fallback to start exactly there. The first probe must fail,
    // and the prober must walk up to a higher port.
    let occupier = ProxyServer::bind(loopback_zero())
        .await
        .expect("occupier bind should succeed");
    let busy_port = occupier.local_addr().port();

    let server = ProxyServer::bind_with_fallback(busy_port, 100)
        .await
        .expect("prober should find an available port within 100 attempts");

    assert_ne!(
        server.local_addr().port(),
        busy_port,
        "prober must not re-use the occupied port",
    );
    // Keep `occupier` alive until the assertion: dropping it earlier
    // would release the port and break the test's invariant.
    drop(occupier);
}
