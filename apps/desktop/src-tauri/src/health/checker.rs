//! T1.0.2.04–08 — [`HealthChecker`] periodic probe loop.
//!
//! Runs a `tokio::interval` that probes every registered provider,
//! updates [`RuntimeState`] health + the per-provider
//! [`SlidingWindow`], and emits Tauri events when health transitions.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Runtime};
use tokio::sync::{watch, RwLock};
use tokio::task::JoinHandle;

use crate::providers::api::{HealthStatus, Provider as ProviderTrait};
use crate::providers::manager::ProviderManager;

use super::sliding_window::SlidingWindow;

/// Default interval between health probes.
pub const DEFAULT_CHECK_INTERVAL: Duration = Duration::from_secs(5);
/// Default sliding window capacity (number of recent probes).
pub const DEFAULT_WINDOW_SIZE: usize = 10;
/// Below this success rate the provider is `Degraded`.
pub const DEGRADED_THRESHOLD: f64 = 0.7;
/// Below this success rate the provider is `Down`.
pub const DOWN_THRESHOLD: f64 = 0.3;
/// Tauri event emitted when a provider health status changes.
pub const EVENT_PROVIDER_STATUS_CHANGED: &str = "provider-status-changed";

/// Payload emitted on `provider-status-changed` (T1.0.2.08).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthChangedEvent {
    pub provider_id: String,
    pub provider_name: String,
    pub old_status: HealthStatus,
    pub new_status: HealthStatus,
    pub success_rate: f64,
}

/// Per-provider probe bookkeeping.
#[derive(Debug)]
struct ProbeState {
    window: SlidingWindow,
    last_status: HealthStatus,
}

/// Cached health snapshot for all providers (T1.0.2.07).
#[derive(Debug, Clone, Serialize)]
pub struct HealthSnapshot {
    pub provider_id: String,
    pub provider_name: String,
    pub status: HealthStatus,
    pub success_rate: f64,
    pub response_time_us: Option<i64>,
}

/// Shared inner state — behind `Arc` so the spawned task can
/// reference it without lifetime issues.
struct Inner {
    manager: Arc<ProviderManager>,
    probe_states: RwLock<HashMap<String, ProbeState>>,
    cache: RwLock<Vec<HealthSnapshot>>,
    event_tx: watch::Sender<Option<HealthChangedEvent>>,
    window_size: usize,
}

/// The health checker owns the probe loop and the status cache.
pub struct HealthChecker {
    inner: Arc<Inner>,
    event_rx: watch::Receiver<Option<HealthChangedEvent>>,
    handle: RwLock<Option<JoinHandle<()>>>,
}

impl std::fmt::Debug for HealthChecker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HealthChecker")
            .field("window_size", &self.inner.window_size)
            .finish_non_exhaustive()
    }
}

impl HealthChecker {
    pub fn new(manager: Arc<ProviderManager>) -> Self {
        Self::with_window_size(manager, DEFAULT_WINDOW_SIZE)
    }

    pub fn with_window_size(manager: Arc<ProviderManager>, window_size: usize) -> Self {
        let (event_tx, event_rx) = watch::channel(None);
        Self {
            inner: Arc::new(Inner {
                manager,
                probe_states: RwLock::new(HashMap::new()),
                cache: RwLock::new(Vec::new()),
                event_tx,
                window_size,
            }),
            event_rx,
            handle: RwLock::new(None),
        }
    }

    /// Subscribe to health-change events.
    pub fn event_receiver(&self) -> watch::Receiver<Option<HealthChangedEvent>> {
        self.event_rx.clone()
    }

    /// Forward health-change events to Tauri windows.
    pub fn forward_events_to_app<R: Runtime>(&self, app: AppHandle<R>) {
        let mut rx = self.event_receiver();
        tauri::async_runtime::spawn(async move {
            while rx.changed().await.is_ok() {
                let Some(event) = rx.borrow_and_update().clone() else {
                    continue;
                };
                emit_provider_status_changed(&app, &event);
            }
        });
    }

    /// Read the cached health snapshot (T1.0.2.07 / T1.0.2.21).
    pub async fn snapshot(&self) -> Vec<HealthSnapshot> {
        let cached = self.inner.cache.read().await.clone();
        if cached.is_empty() {
            return build_snapshot_from_runtime(&self.inner).await;
        }
        cached
    }

    /// Start the periodic probe loop. Idempotent.
    pub async fn start(&self, interval: Duration) {
        let mut guard = self.handle.write().await;
        if guard.is_some() {
            return;
        }
        let inner = Arc::clone(&self.inner);
        let handle = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            ticker.tick().await; // skip immediate first tick
            loop {
                ticker.tick().await;
                run_probe_cycle(&inner).await;
            }
        });
        *guard = Some(handle);
    }

    /// Stop the probe loop. Idempotent.
    pub async fn stop(&self) {
        let mut guard = self.handle.write().await;
        if let Some(handle) = guard.take() {
            handle.abort();
        }
    }

    /// Run one probe cycle immediately (useful for tests and the
    /// initial startup probe).
    pub async fn probe_once(&self) {
        run_probe_cycle(&self.inner).await;
    }
}

/// Single probe cycle: iterate all providers, check health, update
/// windows, emit events on transitions, rebuild the cache.
async fn run_probe_cycle(inner: &Inner) {
    let providers = inner.manager.list().await;
    let mut snapshots = Vec::with_capacity(providers.len());

    for provider in &providers {
        let probe_start = Instant::now();
        let probe_result = provider.health_check().await;
        let elapsed_us = i64::try_from(probe_start.elapsed().as_micros()).unwrap_or(i64::MAX);
        provider.state.record_response_us(elapsed_us);
        let reported_status = match probe_result {
            Ok(status) => status,
            Err(_) => HealthStatus::Down,
        };
        let probe_ok = reported_status != HealthStatus::Down;

        let mut states = inner.probe_states.write().await;
        let state = states
            .entry(provider.id().to_owned())
            .or_insert_with(|| ProbeState {
                window: SlidingWindow::new(inner.window_size),
                last_status: HealthStatus::Healthy,
            });

        state.window.push(probe_ok);
        let rate = state.window.success_rate();
        let new_status = classify_health(reported_status, rate);

        // T1.0.2.08: emit event on transition
        if new_status != state.last_status {
            let event = HealthChangedEvent {
                provider_id: provider.id().to_owned(),
                provider_name: provider.name().to_owned(),
                old_status: state.last_status,
                new_status,
                success_rate: rate,
            };
            let _ = inner.event_tx.send(Some(event));
        }
        state.last_status = new_status;

        // Update the wrapper's RuntimeState so strategies see current
        // health without querying the checker.
        provider.state.set_health(new_status).await;

        snapshots.push(HealthSnapshot {
            provider_id: provider.id().to_owned(),
            provider_name: provider.name().to_owned(),
            status: new_status,
            success_rate: rate,
            response_time_us: provider.state.rolling_response_us(),
        });
    }

    *inner.cache.write().await = snapshots;
}

async fn build_snapshot_from_runtime(inner: &Inner) -> Vec<HealthSnapshot> {
    let providers = inner.manager.list().await;
    let mut snapshots = Vec::with_capacity(providers.len());
    let probe_states = inner.probe_states.read().await;

    for provider in &providers {
        let status = provider.state.health().await;
        let success_rate = probe_states.get(provider.id()).map_or_else(
            || default_success_rate(status),
            |state| state.window.success_rate(),
        );

        snapshots.push(HealthSnapshot {
            provider_id: provider.id().to_owned(),
            provider_name: provider.name().to_owned(),
            status,
            success_rate,
            response_time_us: provider.state.rolling_response_us(),
        });
    }

    snapshots
}

const fn default_success_rate(status: HealthStatus) -> f64 {
    match status {
        HealthStatus::Healthy => 1.0,
        HealthStatus::Degraded => 0.5,
        HealthStatus::Down => 0.0,
    }
}

/// Map success rate to health status using the two thresholds.
fn classify_health(reported_status: HealthStatus, success_rate: f64) -> HealthStatus {
    match reported_status {
        HealthStatus::Degraded => HealthStatus::Degraded,
        HealthStatus::Healthy if success_rate >= DEGRADED_THRESHOLD => HealthStatus::Healthy,
        HealthStatus::Healthy if success_rate >= DOWN_THRESHOLD => HealthStatus::Degraded,
        HealthStatus::Down | HealthStatus::Healthy => HealthStatus::Down,
    }
}

/// Best-effort Tauri event emission. A closed window or missing
/// listener must not break the health loop.
pub fn emit_provider_status_changed<R: Runtime>(app: &AppHandle<R>, event: &HealthChangedEvent) {
    let _ = app.emit(EVENT_PROVIDER_STATUS_CHANGED, event.clone());
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // exact ratios from small integers
mod tests {
    use super::*;
    use crate::providers::api::{
        ApiChoice, ApiRequest, ApiResponse, ApiUsage, ChatMessage, ProviderError, StreamChunk,
        StreamingResponse,
    };
    use crate::providers::model::ProviderKind;
    use crate::providers::wrapper::ProviderWrapper;
    use async_trait::async_trait;
    use futures::stream;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[derive(Debug)]
    struct ConfigurableMockProvider {
        id: String,
        should_succeed: Arc<AtomicBool>,
        degraded_when_succeeding: bool,
    }

    #[async_trait]
    impl ProviderTrait for ConfigurableMockProvider {
        fn id(&self) -> &str {
            &self.id
        }
        fn name(&self) -> &str {
            &self.id
        }
        fn get_priority(&self) -> i32 {
            100
        }
        fn get_cost_per_token(&self) -> Option<f64> {
            None
        }
        fn get_quota_remaining(&self) -> Option<u64> {
            None
        }
        async fn health_check(&self) -> Result<HealthStatus, ProviderError> {
            if self.should_succeed.load(Ordering::Relaxed) {
                if self.degraded_when_succeeding {
                    Ok(HealthStatus::Degraded)
                } else {
                    Ok(HealthStatus::Healthy)
                }
            } else {
                Err(ProviderError::Network("mock failure".into()))
            }
        }
        async fn send_request(&self, _: ApiRequest) -> Result<ApiResponse, ProviderError> {
            Ok(ApiResponse {
                id: "r".into(),
                model: "m".into(),
                choices: vec![ApiChoice {
                    index: 0,
                    message: ChatMessage {
                        role: "assistant".into(),
                        content: "ok".into(),
                        tool_call_id: None,
                        tool_calls: vec![],
                    },
                    finish_reason: Some("stop".into()),
                }],
                usage: Some(ApiUsage {
                    prompt_tokens: 1,
                    completion_tokens: 1,
                    total_tokens: 2,
                }),
            })
        }
        async fn send_stream_request(
            &self,
            _: ApiRequest,
        ) -> Result<StreamingResponse, ProviderError> {
            let s: Pin<Box<dyn futures::Stream<Item = StreamChunk> + Send>> =
                Box::pin(stream::empty());
            Ok(s)
        }
    }

    async fn make_manager_with_mock(succeed: bool) -> (Arc<ProviderManager>, Arc<AtomicBool>) {
        make_manager_with_mock_status(succeed, false).await
    }

    async fn make_manager_with_mock_status(
        succeed: bool,
        degraded_when_succeeding: bool,
    ) -> (Arc<ProviderManager>, Arc<AtomicBool>) {
        let flag = Arc::new(AtomicBool::new(succeed));
        let mock = ConfigurableMockProvider {
            id: "mock1".into(),
            should_succeed: Arc::clone(&flag),
            degraded_when_succeeding,
        };
        let wrapper = Arc::new(ProviderWrapper::new(
            "mock1",
            "Mock Provider",
            ProviderKind::Openai,
            10,
            None,
            true,
            Box::new(mock),
        ));
        let mgr = Arc::new(ProviderManager::new());
        mgr.add(wrapper).await.unwrap();
        (mgr, flag)
    }

    #[test]
    fn classify_health_thresholds() {
        assert_eq!(
            classify_health(HealthStatus::Healthy, 1.0),
            HealthStatus::Healthy
        );
        assert_eq!(
            classify_health(HealthStatus::Healthy, 0.7),
            HealthStatus::Healthy
        );
        assert_eq!(
            classify_health(HealthStatus::Healthy, 0.69),
            HealthStatus::Degraded
        );
        assert_eq!(
            classify_health(HealthStatus::Healthy, 0.3),
            HealthStatus::Degraded
        );
        assert_eq!(
            classify_health(HealthStatus::Healthy, 0.29),
            HealthStatus::Down
        );
        assert_eq!(
            classify_health(HealthStatus::Healthy, 0.0),
            HealthStatus::Down
        );
    }

    #[test]
    fn classify_health_preserves_reported_down_and_degraded_statuses() {
        assert_eq!(classify_health(HealthStatus::Down, 1.0), HealthStatus::Down);
        assert_eq!(
            classify_health(HealthStatus::Degraded, 1.0),
            HealthStatus::Degraded
        );
    }

    #[test]
    fn event_name_is_stable_tauri_wire_id() {
        assert_eq!(EVENT_PROVIDER_STATUS_CHANGED, "provider-status-changed");
    }

    #[test]
    fn health_changed_event_serializes_frontend_wire_payload() {
        let event = HealthChangedEvent {
            provider_id: "provider-1".to_owned(),
            provider_name: "Primary".to_owned(),
            old_status: HealthStatus::Healthy,
            new_status: HealthStatus::Down,
            success_rate: 0.0,
        };

        let raw = serde_json::to_string(&event).expect("json payload");
        let payload: HealthChangedEvent = serde_json::from_str(&raw).expect("json payload");

        assert_eq!(payload.provider_id, "provider-1");
        assert_eq!(payload.old_status, HealthStatus::Healthy);
        assert_eq!(payload.new_status, HealthStatus::Down);
    }

    #[tokio::test]
    async fn probe_once_updates_cache() {
        let (mgr, _flag) = make_manager_with_mock(true).await;
        let checker = HealthChecker::new(mgr);
        checker.probe_once().await;
        let snap = checker.snapshot().await;
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].provider_id, "mock1");
        assert_eq!(snap[0].status, HealthStatus::Healthy);
        assert_eq!(snap[0].success_rate, 1.0);
    }

    #[tokio::test]
    async fn probe_once_records_response_time_sample() {
        let (mgr, _flag) = make_manager_with_mock(true).await;
        let provider = mgr.get("mock1").await.expect("provider");
        let checker = HealthChecker::new(mgr);

        checker.probe_once().await;

        assert!(provider.state.rolling_response_us().is_some());
    }

    #[tokio::test]
    async fn probe_once_preserves_provider_reported_degraded_status() {
        let (mgr, _flag) = make_manager_with_mock_status(true, true).await;
        let checker = HealthChecker::new(mgr);

        checker.probe_once().await;
        let snap = checker.snapshot().await;

        assert_eq!(snap[0].status, HealthStatus::Degraded);
    }

    #[tokio::test]
    async fn snapshot_without_probe_reads_runtime_providers() {
        let (mgr, _flag) = make_manager_with_mock(true).await;
        let checker = HealthChecker::new(mgr);

        let snap = checker.snapshot().await;

        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].provider_id, "mock1");
        assert_eq!(snap[0].status, HealthStatus::Healthy);
        assert_eq!(snap[0].success_rate, 1.0);
    }

    #[tokio::test]
    async fn snapshot_without_probe_reflects_runtime_health() {
        let (mgr, _flag) = make_manager_with_mock(true).await;
        let provider = mgr.get("mock1").await.expect("provider");
        provider.state.set_health(HealthStatus::Down).await;
        let checker = HealthChecker::new(mgr);

        let snap = checker.snapshot().await;

        assert_eq!(snap[0].status, HealthStatus::Down);
        assert_eq!(snap[0].success_rate, 0.0);
    }

    #[tokio::test]
    async fn probe_failure_degrades_status() {
        let (mgr, flag) = make_manager_with_mock(false).await;
        let checker = HealthChecker::with_window_size(mgr, 3);
        // 3 failures → success_rate = 0.0 → Down
        checker.probe_once().await;
        checker.probe_once().await;
        checker.probe_once().await;
        let snap = checker.snapshot().await;
        assert_eq!(snap[0].status, HealthStatus::Down);
        assert_eq!(snap[0].success_rate, 0.0);

        // Recover: 3 successes fill the window
        flag.store(true, Ordering::Relaxed);
        checker.probe_once().await;
        checker.probe_once().await;
        checker.probe_once().await;
        let snap = checker.snapshot().await;
        assert_eq!(snap[0].status, HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn health_change_emits_event() {
        let (mgr, flag) = make_manager_with_mock(true).await;
        let checker = HealthChecker::with_window_size(mgr, 2);
        let mut rx = checker.event_receiver();

        // First probe: Healthy (no change from default)
        checker.probe_once().await;

        // Second probe (fail): transport failure immediately removes
        // the provider from strategy selection.
        flag.store(false, Ordering::Relaxed);
        checker.probe_once().await;

        rx.changed().await.unwrap();
        let event = rx.borrow_and_update().clone().expect("event");
        assert_eq!(event.provider_id, "mock1");
        assert_eq!(event.old_status, HealthStatus::Healthy);
        assert_eq!(event.new_status, HealthStatus::Down);
    }

    #[tokio::test]
    async fn start_and_stop_idempotent() {
        let (mgr, _) = make_manager_with_mock(true).await;
        let checker = HealthChecker::new(mgr);
        checker.start(Duration::from_secs(3600)).await;
        checker.start(Duration::from_secs(3600)).await; // idempotent
        checker.stop().await;
        checker.stop().await; // idempotent
    }
}
