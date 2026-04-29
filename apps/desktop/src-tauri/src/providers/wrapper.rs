//! [`ProviderWrapper`] — the `SwitchEngine`'s view of a provider.
//!
//! Combines the persisted config ([`model::Provider`]), the runtime
//! [`api::Provider`] trait impl (e.g. `OpenAIProvider`), and mutable
//! health/performance metrics that strategies inspect.

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use tokio::sync::RwLock;

use super::api::{
    ApiRequest, ApiResponse, HealthStatus, Provider, ProviderError, StreamingResponse,
};
use super::model::ProviderKind;

/// Runtime state that strategies read and the `HealthChecker` /
/// request pipeline write. All fields are lock-free atomics or
/// interior-mutable to avoid contention on the hot path.
#[derive(Debug)]
pub struct RuntimeState {
    /// Last observed health status.
    health: RwLock<HealthStatus>,
    /// Rolling average response time in microseconds (`-1` = no data).
    rolling_response_us: AtomicI64,
    /// Remaining quota reported by the upstream (0 = unknown).
    quota_remaining: AtomicU64,
}

impl RuntimeState {
    fn new() -> Self {
        Self {
            health: RwLock::new(HealthStatus::Healthy),
            rolling_response_us: AtomicI64::new(-1),
            quota_remaining: AtomicU64::new(0),
        }
    }

    pub async fn health(&self) -> HealthStatus {
        *self.health.read().await
    }

    pub async fn set_health(&self, status: HealthStatus) {
        *self.health.write().await = status;
    }

    /// Rolling response time in microseconds, or `None` if no sample
    /// has been recorded yet.
    pub fn rolling_response_us(&self) -> Option<i64> {
        let v = self.rolling_response_us.load(Ordering::Relaxed);
        if v < 0 {
            None
        } else {
            Some(v)
        }
    }

    /// Exponentially weighted moving average update. `alpha` is fixed
    /// at 0.3 — recent samples weigh more but old history still
    /// damps spikes.
    pub fn record_response_us(&self, us: i64) {
        let prev = self.rolling_response_us.load(Ordering::Relaxed);
        let next = if prev < 0 {
            us
        } else {
            // EWMA: next = alpha * sample + (1 - alpha) * prev
            // Using integer math: (3 * us + 7 * prev) / 10
            (3 * us + 7 * prev) / 10
        };
        self.rolling_response_us.store(next, Ordering::Relaxed);
    }

    pub fn quota_remaining(&self) -> Option<u64> {
        let v = self.quota_remaining.load(Ordering::Relaxed);
        if v == 0 {
            None
        } else {
            Some(v)
        }
    }

    pub fn set_quota_remaining(&self, remaining: u64) {
        self.quota_remaining.store(remaining, Ordering::Relaxed);
    }
}

/// Wraps a concrete [`Provider`] with config and runtime metrics.
/// The `SwitchEngine` holds `Vec<Arc<ProviderWrapper>>` — the `Arc`
/// lets the health checker and request pipeline update state
/// concurrently.
pub struct ProviderWrapper {
    /// Database row id.
    id: String,
    /// Display name.
    name: String,
    /// Protocol family.
    kind: ProviderKind,
    /// Lower = preferred.
    priority: i32,
    /// Per-token cost in USD; `None` = unknown.
    cost_per_token: Option<f64>,
    /// Whether the user has enabled this provider.
    enabled: bool,
    /// The underlying runtime provider (HTTP client).
    inner: Box<dyn Provider>,
    /// Mutable runtime metrics.
    pub state: Arc<RuntimeState>,
}

impl std::fmt::Debug for ProviderWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderWrapper")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("kind", &self.kind)
            .field("priority", &self.priority)
            .field("enabled", &self.enabled)
            .finish_non_exhaustive()
    }
}

impl ProviderWrapper {
    /// Construct from a concrete provider and its persisted config.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        kind: ProviderKind,
        priority: i32,
        cost_per_token: Option<f64>,
        enabled: bool,
        inner: Box<dyn Provider>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind,
            priority,
            cost_per_token,
            enabled,
            inner,
            state: Arc::new(RuntimeState::new()),
        }
    }

    pub fn kind(&self) -> ProviderKind {
        self.kind
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn set_priority(&mut self, priority: i32) {
        self.priority = priority;
    }

    pub fn set_cost_per_token(&mut self, cost: Option<f64>) {
        self.cost_per_token = cost;
    }

    /// Runtime state handle — shared with `HealthChecker`.
    pub fn runtime_state(&self) -> Arc<RuntimeState> {
        Arc::clone(&self.state)
    }
}

#[async_trait]
impl Provider for ProviderWrapper {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn get_priority(&self) -> i32 {
        self.priority
    }

    fn get_cost_per_token(&self) -> Option<f64> {
        self.cost_per_token
    }

    fn get_quota_remaining(&self) -> Option<u64> {
        self.state.quota_remaining()
    }

    async fn health_check(&self) -> Result<HealthStatus, ProviderError> {
        let result = self.inner.health_check().await;
        if let Ok(status) = &result {
            self.state.set_health(*status).await;
        }
        result
    }

    async fn send_request(&self, request: ApiRequest) -> Result<ApiResponse, ProviderError> {
        let start = Instant::now();
        let result = self.inner.send_request(request).await;
        let elapsed_us = i64::try_from(start.elapsed().as_micros()).unwrap_or(i64::MAX);
        self.state.record_response_us(elapsed_us);
        result
    }

    async fn send_stream_request(
        &self,
        request: ApiRequest,
    ) -> Result<StreamingResponse, ProviderError> {
        let start = Instant::now();
        let result = self.inner.send_stream_request(request).await;
        // Record TTFB (time to first byte = time until the stream is
        // handed back). Actual per-chunk latency is a future metric.
        let elapsed_us = i64::try_from(start.elapsed().as_micros()).unwrap_or(i64::MAX);
        self.state.record_response_us(elapsed_us);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::api::{ApiChoice, ApiUsage, ChatMessage, StreamChunk};
    use futures::stream;
    use std::pin::Pin;

    /// Minimal mock provider for unit tests.
    #[derive(Debug)]
    struct MockProvider {
        id: String,
    }

    #[async_trait]
    impl Provider for MockProvider {
        fn id(&self) -> &str {
            &self.id
        }
        fn name(&self) -> &str {
            self.id.as_str()
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
            Ok(HealthStatus::Healthy)
        }
        async fn send_request(&self, _req: ApiRequest) -> Result<ApiResponse, ProviderError> {
            Ok(ApiResponse {
                id: "resp-1".into(),
                model: "mock-model".into(),
                choices: vec![ApiChoice {
                    index: 0,
                    message: ChatMessage {
                        role: "assistant".into(),
                        content: "ok".into(),
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
            _req: ApiRequest,
        ) -> Result<StreamingResponse, ProviderError> {
            let s: Pin<Box<dyn futures::Stream<Item = StreamChunk> + Send>> =
                Box::pin(stream::empty());
            Ok(s)
        }
    }

    fn sample_request() -> ApiRequest {
        ApiRequest {
            model: "m".into(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: "hi".into(),
            }],
            temperature: None,
            max_tokens: None,
            stream: false,
            tools: vec![],
        }
    }

    fn make_wrapper() -> ProviderWrapper {
        ProviderWrapper::new(
            "p1",
            "Test Provider",
            ProviderKind::Openai,
            10,
            Some(0.000_003),
            true,
            Box::new(MockProvider { id: "p1".into() }),
        )
    }

    #[test]
    fn wrapper_exposes_config_via_trait_getters() {
        let w = make_wrapper();
        assert_eq!(w.id(), "p1");
        assert_eq!(w.name(), "Test Provider");
        assert_eq!(w.get_priority(), 10);
        assert_eq!(w.get_cost_per_token(), Some(0.000_003));
        assert_eq!(w.get_quota_remaining(), None);
        assert!(w.is_enabled());
        assert_eq!(w.kind(), ProviderKind::Openai);
    }

    #[tokio::test]
    async fn health_check_updates_runtime_state() {
        let w = make_wrapper();
        assert_eq!(w.state.health().await, HealthStatus::Healthy);
        let status = w.health_check().await.expect("ok");
        assert_eq!(status, HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn send_request_records_response_time() {
        let w = make_wrapper();
        assert!(w.state.rolling_response_us().is_none());
        w.send_request(sample_request()).await.expect("ok");
        assert!(w.state.rolling_response_us().is_some());
    }

    #[tokio::test]
    async fn send_stream_request_records_ttfb() {
        let w = make_wrapper();
        assert!(w.state.rolling_response_us().is_none());
        let _stream = w.send_stream_request(sample_request()).await.expect("ok");
        assert!(w.state.rolling_response_us().is_some());
    }

    #[test]
    fn runtime_state_ewma_converges() {
        let state = RuntimeState::new();
        // First sample initialises directly.
        state.record_response_us(1000);
        assert_eq!(state.rolling_response_us(), Some(1000));
        // Second sample: EWMA(3/10 * 2000 + 7/10 * 1000) = 1300
        state.record_response_us(2000);
        assert_eq!(state.rolling_response_us(), Some(1300));
    }

    #[test]
    fn runtime_state_quota_default_is_none() {
        let state = RuntimeState::new();
        assert_eq!(state.quota_remaining(), None);
        state.set_quota_remaining(500);
        assert_eq!(state.quota_remaining(), Some(500));
    }

    #[test]
    fn set_priority_and_cost_mutate_wrapper() {
        let mut w = make_wrapper();
        w.set_priority(1);
        assert_eq!(w.get_priority(), 1);
        w.set_cost_per_token(Some(0.000_01));
        assert_eq!(w.get_cost_per_token(), Some(0.000_01));
    }

    #[test]
    fn debug_does_not_leak_inner_details() {
        let w = make_wrapper();
        let rendered = format!("{w:?}");
        assert!(rendered.contains("ProviderWrapper"));
        assert!(rendered.contains("p1"));
    }
}
