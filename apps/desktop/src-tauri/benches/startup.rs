//! T1.0.5.01 — Startup time profiling benchmark.
//!
//! Measures the core startup sequence that runs before the UI appears:
//! 1. Database open + PRAGMA configuration
//! 2. Schema migrations
//! 3. Master key generation (file-based backend)
//! 4. Proxy server bind
//!
//! Target: all four operations combined complete in < 500 ms.

#![allow(unused_imports)]

use std::sync::Arc;

use criterion::{criterion_group, criterion_main, Criterion};
use tempfile::TempDir;

use ccuse_desktop_lib::crypto::{load_or_create_master_key, FileKeyringBackend};
use ccuse_desktop_lib::db::{open_database, run_migrations};
use ccuse_desktop_lib::proxy::ProxyServer;

fn bench_db_open(c: &mut Criterion) {
    c.bench_function("db_open", |b| {
        b.iter_with_setup(
            || {
                let dir = TempDir::new().expect("tempdir");
                dir.path().join("bench.db")
            },
            |path| {
                let _db = open_database(&path).expect("open");
            },
        );
    });
}

fn bench_migrations(c: &mut Criterion) {
    c.bench_function("migrations", |b| {
        b.iter_with_setup(
            || {
                let dir = TempDir::new().expect("tempdir");
                let path = dir.path().join("bench.db");
                let db = open_database(&path).expect("open");
                (dir, db)
            },
            |(_dir, db)| {
                run_migrations(&db).expect("migrate");
            },
        );
    });
}

fn bench_master_key_init(c: &mut Criterion) {
    c.bench_function("master_key_init", |b| {
        b.iter_with_setup(
            || {
                let dir = TempDir::new().expect("tempdir");
                let path = dir.path().join("keyring_fallback.json");
                (dir, path)
            },
            |(_dir, path)| {
                let backend = FileKeyringBackend::new(&path);
                let _key = load_or_create_master_key(&backend).expect("key");
            },
        );
    });
}

fn bench_proxy_bind(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    c.bench_function("proxy_bind", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _server = ProxyServer::bind(([127, 0, 0, 1], 0).into())
                    .await
                    .expect("bind");
            });
        });
    });
}

fn bench_full_startup_sequence(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    c.bench_function("full_startup_sequence", |b| {
        b.iter_with_setup(
            || TempDir::new().expect("tempdir"),
            |dir| {
                let db_path = dir.path().join("bench.db");
                let db = open_database(&db_path).expect("open");
                run_migrations(&db).expect("migrate");

                let keyring_path = dir.path().join("keyring_fallback.json");
                let backend = FileKeyringBackend::new(keyring_path);
                let _key = Arc::new(load_or_create_master_key(&backend).expect("key"));

                rt.block_on(async {
                    let _server = ProxyServer::bind(([127, 0, 0, 1], 0).into())
                        .await
                        .expect("bind");
                });
            },
        );
    });
}

criterion_group!(
    benches,
    bench_db_open,
    bench_migrations,
    bench_master_key_init,
    bench_proxy_bind,
    bench_full_startup_sequence,
);
criterion_main!(benches);
