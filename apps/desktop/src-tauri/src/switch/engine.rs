//! T1.0.2.15–16 — [`SwitchEngine`] retry chain + error classification.
//!
//! The engine wraps strategy selection with a retry loop: on a
//! retriable failure it marks the provider `Degraded`, picks the next
//! candidate, and retries — up to `max_retries` times.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::providers::api::{
    ApiRequest, ApiResponse, HealthStatus, Provider as ProviderTrait, ProviderError,
    StreamingResponse,
};
use crate::providers::manager::ProviderManager;
use crate::providers::wrapper::ProviderWrapper;

use super::strategy::{select, RoundRobinState, SmartWeights, SwitchStrategy};

/// Default maximum retries before returning an error to the caller.
pub const DEFAULT_MAX_RETRIES: usize = 3;

/// Result of a switch+dispatch operation.
#[derive(Debug)]
pub struct DispatchResult<T> {
    /// The provider that actually served the request.
    pub provider_id: String,
    pub provider_name: String,
    /// Number of attempts before success (1 = no retry).
    pub attempts: usize,
    /// The upstream response.
    pub response: T,
}

/// Configuration for the switch engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchConfig {
    pub strategy: SwitchStrategy,
    pub max_retries: usize,
    pub smart_weights: SmartWeights,
}

impl Default for SwitchConfig {
    fn default() -> Self {
        Self {
            strategy: SwitchStrategy::Priority,
            max_retries: DEFAULT_MAX_RETRIES,
            smart_weights: SmartWeights::default(),
        }
    }
}

/// Core switch engine. Owns a `ProviderManager` and dispatches
/// requests through the active strategy with automatic failover.
pub struct SwitchEngine {
    manager: Arc<ProviderManager>,
    config: RwLock<SwitchConfig>,
    rr_state: RoundRobinState,
}

impl std::fmt::Debug for SwitchEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SwitchEngine").finish_non_exhaustive()
    }
}

impl SwitchEngine {
    pub fn new(manager: Arc<ProviderManager>) -> Self {
        Self::with_config(manager, SwitchConfig::default())
    }

    pub fn with_config(manager: Arc<ProviderManager>, config: SwitchConfig) -> Self {
        Self {
            manager,
            config: RwLock::new(config),
            rr_state: RoundRobinState::default(),
        }
    }

    /// Update strategy at runtime.
    pub async fn set_strategy(&self, strategy: SwitchStrategy) {
        self.config.write().await.strategy = strategy;
    }

    /// Read current strategy.
    pub async fn strategy(&self) -> SwitchStrategy {
        self.config.read().await.strategy
    }

    /// Update smart weights.
    pub async fn set_smart_weights(&self, weights: SmartWeights) {
        self.config.write().await.smart_weights = weights;
    }

    /// Update max retries.
    pub async fn set_max_retries(&self, max: usize) {
        self.config.write().await.max_retries = max;
    }

    /// Read current config snapshot.
    pub async fn config(&self) -> SwitchConfig {
        self.config.read().await.clone()
    }

    /// Dispatch a non-streaming request with failover.
    pub async fn dispatch(
        &self,
        request: ApiRequest,
    ) -> Result<DispatchResult<ApiResponse>, ProviderError> {
        self.dispatch_with_request_mapper(request, clone_request_for_provider)
            .await
    }

    /// Dispatch a non-streaming request, allowing callers to adjust the
    /// provider-layer request after a concrete provider is selected.
    pub async fn dispatch_with_request_mapper<F>(
        &self,
        request: ApiRequest,
        map_request: F,
    ) -> Result<DispatchResult<ApiResponse>, ProviderError>
    where
        F: Fn(&ApiRequest, &ProviderWrapper) -> ApiRequest + Send + Sync,
    {
        let config = self.config.read().await.clone();
        let candidates = self.manager.enabled_by_priority().await;

        if candidates.is_empty() {
            return Err(ProviderError::BadRequest(
                "no enabled providers configured".into(),
            ));
        }

        let mut tried: Vec<String> = Vec::new();
        let mut last_error: Option<ProviderError> = None;

        for attempt in 0..=config.max_retries {
            let candidate = select_excluding(
                config.strategy,
                &candidates,
                &self.rr_state,
                &config.smart_weights,
                &tried,
            );

            let Some(provider) = candidate else { break };

            tried.push(provider.id().to_owned());

            let provider_request = map_request(&request, provider.as_ref());
            match provider.send_request(provider_request).await {
                Ok(response) => {
                    return Ok(DispatchResult {
                        provider_id: provider.id().to_owned(),
                        provider_name: provider.name().to_owned(),
                        attempts: attempt + 1,
                        response,
                    });
                }
                Err(err) => {
                    if err.is_retriable() {
                        provider.state.set_health(HealthStatus::Degraded).await;
                    }
                    last_error = Some(err);
                    if !last_error.as_ref().is_some_and(ProviderError::is_retriable) {
                        break;
                    }
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| ProviderError::BadRequest("all providers exhausted".into())))
    }

    /// Dispatch a streaming request with failover. Retry only
    /// applies to the initial connection — once the stream starts,
    /// mid-stream failures are surfaced to the caller.
    pub async fn dispatch_stream(
        &self,
        request: ApiRequest,
    ) -> Result<DispatchResult<StreamingResponse>, ProviderError> {
        self.dispatch_stream_with_request_mapper(request, clone_request_for_provider)
            .await
    }

    /// Dispatch a streaming request, allowing callers to adjust the
    /// provider-layer request after a concrete provider is selected.
    pub async fn dispatch_stream_with_request_mapper<F>(
        &self,
        request: ApiRequest,
        map_request: F,
    ) -> Result<DispatchResult<StreamingResponse>, ProviderError>
    where
        F: Fn(&ApiRequest, &ProviderWrapper) -> ApiRequest + Send + Sync,
    {
        let config = self.config.read().await.clone();
        let candidates = self.manager.enabled_by_priority().await;

        if candidates.is_empty() {
            return Err(ProviderError::BadRequest(
                "no enabled providers configured".into(),
            ));
        }

        let mut tried: Vec<String> = Vec::new();
        let mut last_error: Option<ProviderError> = None;

        for attempt in 0..=config.max_retries {
            let candidate = select_excluding(
                config.strategy,
                &candidates,
                &self.rr_state,
                &config.smart_weights,
                &tried,
            );

            let Some(provider) = candidate else { break };

            tried.push(provider.id().to_owned());

            let provider_request = map_request(&request, provider.as_ref());
            match provider.send_stream_request(provider_request).await {
                Ok(stream) => {
                    return Ok(DispatchResult {
                        provider_id: provider.id().to_owned(),
                        provider_name: provider.name().to_owned(),
                        attempts: attempt + 1,
                        response: stream,
                    });
                }
                Err(err) => {
                    if err.is_retriable() {
                        provider.state.set_health(HealthStatus::Degraded).await;
                    }
                    last_error = Some(err);
                    if !last_error.as_ref().is_some_and(ProviderError::is_retriable) {
                        break;
                    }
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| ProviderError::BadRequest("all providers exhausted".into())))
    }
}

fn clone_request_for_provider(request: &ApiRequest, _: &ProviderWrapper) -> ApiRequest {
    request.clone()
}

/// Select a provider, excluding those already tried.
fn select_excluding(
    strategy: SwitchStrategy,
    candidates: &[Arc<ProviderWrapper>],
    rr_state: &RoundRobinState,
    weights: &SmartWeights,
    exclude: &[String],
) -> Option<Arc<ProviderWrapper>> {
    let filtered: Vec<_> = candidates
        .iter()
        .filter(|p| !exclude.contains(&p.id().to_owned()))
        .cloned()
        .collect();
    select(strategy, &filtered, rr_state, weights)
}

/// T1.0.2.16: error classification. `is_retriable` is already on
/// [`ProviderError`]; this module re-exports the concept for
/// documentation. The mapping is:
///
/// | HTTP status     | Error variant  | Retriable? |
/// |-----------------|----------------|------------|
/// | 408, 429, 5xx   | Network/Rate/Up| Yes        |
/// | Timeout         | Network        | Yes        |
/// | 400, 401, 403   | BadReq/Unauth  | No         |
/// | 404, 422        | `BadRequest`   | No         |
/// | Decode failures | Decode         | No         |
///
/// The `SwitchEngine` uses `is_retriable` to decide whether to try
/// the next provider or immediately surface the error.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::api::{ApiChoice, ApiRequest, ApiUsage, ChatMessage, StreamChunk};
    use crate::providers::model::ProviderKind;
    use async_trait::async_trait;
    use futures::stream;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering as AtomicOrdering};
    use std::sync::Mutex;

    #[derive(Debug)]
    struct MockDispatchProvider {
        id: String,
        should_succeed: Arc<AtomicBool>,
        call_count: Arc<AtomicUsize>,
        seen_models: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl ProviderTrait for MockDispatchProvider {
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
            Ok(HealthStatus::Healthy)
        }
        async fn send_request(&self, request: ApiRequest) -> Result<ApiResponse, ProviderError> {
            self.call_count.fetch_add(1, AtomicOrdering::Relaxed);
            self.seen_models
                .lock()
                .expect("record models")
                .push(request.model);
            if self.should_succeed.load(AtomicOrdering::Relaxed) {
                Ok(ApiResponse {
                    id: format!("resp-{}", self.id),
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
            } else {
                Err(ProviderError::Upstream {
                    status: 500,
                    body: "mock 500".into(),
                })
            }
        }
        async fn send_stream_request(
            &self,
            request: ApiRequest,
        ) -> Result<StreamingResponse, ProviderError> {
            self.call_count.fetch_add(1, AtomicOrdering::Relaxed);
            self.seen_models
                .lock()
                .expect("record models")
                .push(request.model);
            if self.should_succeed.load(AtomicOrdering::Relaxed) {
                Ok(Box::pin(stream::empty())
                    as Pin<Box<dyn futures::Stream<Item = StreamChunk> + Send>>)
            } else {
                Err(ProviderError::Upstream {
                    status: 502,
                    body: "mock 502".into(),
                })
            }
        }
    }

    fn sample_request() -> ApiRequest {
        ApiRequest {
            model: "m".into(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: "hi".into(),
                tool_call_id: None,
                tool_calls: vec![],
            }],
            temperature: None,
            max_tokens: None,
            stream: false,
            tools: vec![],
        }
    }

    async fn make_engine(providers: Vec<(String, bool)>) -> (SwitchEngine, Vec<Arc<AtomicUsize>>) {
        let (engine, counters, _) = make_engine_with_model_records(providers).await;
        (engine, counters)
    }

    async fn make_engine_with_model_records(
        providers: Vec<(String, bool)>,
    ) -> (
        SwitchEngine,
        Vec<Arc<AtomicUsize>>,
        Vec<Arc<Mutex<Vec<String>>>>,
    ) {
        let mgr = Arc::new(ProviderManager::new());
        let mut counters = Vec::new();
        let mut seen_models = Vec::new();
        for (id, succeed) in providers {
            let count = Arc::new(AtomicUsize::new(0));
            let models = Arc::new(Mutex::new(Vec::new()));
            let mock = MockDispatchProvider {
                id: id.clone(),
                should_succeed: Arc::new(AtomicBool::new(succeed)),
                call_count: Arc::clone(&count),
                seen_models: Arc::clone(&models),
            };
            let wrapper = Arc::new(ProviderWrapper::new(
                &id,
                &id,
                ProviderKind::Openai,
                10,
                None,
                true,
                Box::new(mock),
            ));
            mgr.add(wrapper).await.unwrap();
            counters.push(count);
            seen_models.push(models);
        }
        (SwitchEngine::new(mgr), counters, seen_models)
    }

    #[tokio::test]
    async fn dispatch_succeeds_on_first_try() {
        let (engine, counters) = make_engine(vec![("p1".into(), true)]).await;
        let result = engine.dispatch(sample_request()).await.unwrap();
        assert_eq!(result.provider_id, "p1");
        assert_eq!(result.attempts, 1);
        assert_eq!(counters[0].load(AtomicOrdering::Relaxed), 1);
    }

    #[tokio::test]
    async fn dispatch_fails_over_to_next_provider() {
        let (engine, counters) = make_engine(vec![("p1".into(), false), ("p2".into(), true)]).await;
        let result = engine.dispatch(sample_request()).await.unwrap();
        assert_eq!(result.provider_id, "p2");
        assert_eq!(result.attempts, 2);
        assert_eq!(counters[0].load(AtomicOrdering::Relaxed), 1);
        assert_eq!(counters[1].load(AtomicOrdering::Relaxed), 1);
    }

    #[tokio::test]
    async fn dispatch_with_request_mapper_applies_selected_provider_model() {
        let (engine, _, seen_models) =
            make_engine_with_model_records(vec![("p1".into(), true)]).await;

        engine
            .dispatch_with_request_mapper(sample_request(), |request, provider| {
                let mut mapped = request.clone();
                mapped.model = format!("{}::{}", provider.id(), request.model);
                mapped
            })
            .await
            .expect("dispatch with mapped model");

        assert_eq!(
            seen_models[0].lock().expect("seen models").as_slice(),
            &["p1::m"],
        );
    }

    #[tokio::test]
    async fn dispatch_all_fail_returns_last_error() {
        let (engine, _) = make_engine(vec![("p1".into(), false), ("p2".into(), false)]).await;
        let err = engine.dispatch(sample_request()).await.unwrap_err();
        assert!(err.is_retriable());
    }

    #[tokio::test]
    async fn dispatch_no_providers_returns_bad_request() {
        let mgr = Arc::new(ProviderManager::new());
        let engine = SwitchEngine::new(mgr);
        let err = engine.dispatch(sample_request()).await.unwrap_err();
        assert!(matches!(err, ProviderError::BadRequest(_)));
    }

    #[tokio::test]
    async fn dispatch_stream_succeeds() {
        let (engine, _) = make_engine(vec![("p1".into(), true)]).await;
        let result = engine.dispatch_stream(sample_request()).await.unwrap();
        assert_eq!(result.provider_id, "p1");
        assert_eq!(result.attempts, 1);
    }

    #[tokio::test]
    async fn set_strategy_takes_effect() {
        let (engine, _) = make_engine(vec![("p1".into(), true)]).await;
        assert_eq!(engine.strategy().await, SwitchStrategy::Priority);
        engine.set_strategy(SwitchStrategy::Smart).await;
        assert_eq!(engine.strategy().await, SwitchStrategy::Smart);
    }

    #[tokio::test]
    async fn failover_marks_degraded() {
        let (engine, _) = make_engine(vec![("p1".into(), false), ("p2".into(), true)]).await;
        engine.dispatch(sample_request()).await.unwrap();
        // p1 should have been marked Degraded
        let p1 = engine.manager.get("p1").await.unwrap();
        let health = p1.state.health().await;
        assert_eq!(health, HealthStatus::Degraded);
    }
}
