//! T1.0.5.03 — Proxy QPS benchmark.
//!
//! Measures the local proxy server's request handling throughput:
//! * Starts a proxy on an ephemeral port
//! * Fires N requests at `/healthz` (lightweight, no upstream needed)
//! * Reports throughput in requests/second
//!
//! Target: >= 200 RPS on a single core (in practice much higher since
//! `/healthz` is a trivial handler with no I/O).

#![allow(unused_imports)]

use std::time::Duration;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use tokio::sync::oneshot;

use std::sync::Arc;

use ccuse_desktop_lib::commands::model_mapping::ModelMappingHandle;
use ccuse_desktop_lib::commands::switch::SwitchEngineHandle;
use ccuse_desktop_lib::converter::ModelMapping;
use ccuse_desktop_lib::providers::ProviderManager;
use ccuse_desktop_lib::proxy::{ProxyAppState, ProxyServer};
use ccuse_desktop_lib::switch::SwitchEngine;
use tokio::sync::RwLock;

fn bench_healthz_throughput(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");

    let (base_url, shutdown_tx, _handle) = rt.block_on(async {
        let server = ProxyServer::bind(([127, 0, 0, 1], 0).into())
            .await
            .expect("bind");
        let base = format!("http://{}", server.local_addr());
        let (tx, rx) = oneshot::channel::<()>();
        let manager = Arc::new(ProviderManager::new());
        let engine: SwitchEngineHandle = Arc::new(SwitchEngine::new(Arc::clone(&manager)));
        let model_mapping: ModelMappingHandle = Arc::new(RwLock::new(ModelMapping::new()));
        let state = ProxyAppState::new(engine, model_mapping, manager);
        let handle = tokio::spawn(server.serve_with_shutdown(state, async move {
            let _ = rx.await;
        }));
        tokio::time::sleep(Duration::from_millis(50)).await;
        (base, tx, handle)
    });

    let client = reqwest::Client::builder()
        .pool_max_idle_per_host(32)
        .build()
        .expect("http client");

    let url = format!("{base_url}/healthz");

    let mut group = c.benchmark_group("proxy_throughput");
    group.throughput(Throughput::Elements(1));
    group.measurement_time(Duration::from_secs(5));

    group.bench_function("healthz_serial", |b| {
        b.to_async(&rt).iter(|| {
            let client = client.clone();
            let url = url.clone();
            async move {
                let resp = client.get(&url).send().await.expect("request");
                assert_eq!(resp.status(), reqwest::StatusCode::OK);
            }
        });
    });

    group.bench_function(BenchmarkId::new("healthz_concurrent", "10"), |b| {
        b.to_async(&rt).iter(|| {
            let client = client.clone();
            let url = url.clone();
            async move {
                let mut handles = Vec::with_capacity(10);
                for _ in 0..10 {
                    let c = client.clone();
                    let u = url.clone();
                    handles.push(tokio::spawn(async move {
                        let resp = c.get(&u).send().await.expect("request");
                        assert_eq!(resp.status(), reqwest::StatusCode::OK);
                    }));
                }
                for h in handles {
                    h.await.expect("join");
                }
            }
        });
    });

    group.finish();

    let _ = shutdown_tx.send(());
}

criterion_group!(benches, bench_healthz_throughput);
criterion_main!(benches);
