//! T1.0.2.21 — Health snapshot Tauri command.
//!
//! `get_health_snapshot` returns all providers' current health status.

use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::health::{HealthChecker, HealthSnapshot};

/// Managed state type for the health checker.
pub type HealthCheckerHandle = Arc<HealthChecker>;

/// JSON-friendly snapshot for the frontend.
#[derive(Debug, Serialize)]
pub struct HealthSnapshotResponse {
    pub providers: Vec<HealthSnapshot>,
}

/// Return the cached health snapshot for all providers.
#[tauri::command]
pub async fn get_health_snapshot(
    checker: State<'_, HealthCheckerHandle>,
) -> Result<HealthSnapshotResponse, String> {
    let providers = checker.snapshot().await;
    Ok(HealthSnapshotResponse { providers })
}

/// Run an immediate probe cycle and return the refreshed health
/// snapshot. Used by manual UI refresh actions.
#[tauri::command]
pub async fn refresh_health_snapshot(
    checker: State<'_, HealthCheckerHandle>,
) -> Result<HealthSnapshotResponse, String> {
    Ok(refresh_health_snapshot_for_checker(&checker).await)
}

async fn refresh_health_snapshot_for_checker(checker: &HealthChecker) -> HealthSnapshotResponse {
    checker.probe_once().await;
    HealthSnapshotResponse {
        providers: checker.snapshot().await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::api::{
        ApiRequest, ApiResponse, HealthStatus, Provider, ProviderError, StreamingResponse,
    };
    use crate::providers::model::ProviderKind;
    use crate::providers::wrapper::ProviderWrapper;
    use crate::providers::ProviderManager;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug)]
    struct CountingHealthProvider {
        id: String,
        name: String,
        calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Provider for CountingHealthProvider {
        fn id(&self) -> &str {
            self.id.as_str()
        }

        fn name(&self) -> &str {
            self.name.as_str()
        }

        fn get_priority(&self) -> i32 {
            10
        }

        fn get_cost_per_token(&self) -> Option<f64> {
            None
        }

        fn get_quota_remaining(&self) -> Option<u64> {
            None
        }

        async fn health_check(&self) -> Result<HealthStatus, ProviderError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Ok(HealthStatus::Degraded)
        }

        async fn send_request(&self, _: ApiRequest) -> Result<ApiResponse, ProviderError> {
            Err(ProviderError::BadRequest("not used".into()))
        }

        async fn send_stream_request(
            &self,
            _: ApiRequest,
        ) -> Result<StreamingResponse, ProviderError> {
            Err(ProviderError::BadRequest("not used".into()))
        }
    }

    #[tokio::test]
    async fn empty_manager_returns_empty_snapshot() {
        let mgr = Arc::new(ProviderManager::new());
        let checker = Arc::new(HealthChecker::new(mgr));
        let snap = checker.snapshot().await;
        assert!(snap.is_empty());
    }

    #[tokio::test]
    async fn refresh_health_snapshot_runs_probe_cycle() {
        let calls = Arc::new(AtomicUsize::new(0));
        let wrapper = Arc::new(ProviderWrapper::new(
            "counting",
            "Counting Provider",
            ProviderKind::Openai,
            10,
            None,
            true,
            Box::new(CountingHealthProvider {
                id: "counting".into(),
                name: "Counting Provider".into(),
                calls: Arc::clone(&calls),
            }),
        ));
        let mgr = Arc::new(ProviderManager::new());
        mgr.add(wrapper).await.expect("add provider");
        let checker = HealthChecker::new(mgr);

        let snap = refresh_health_snapshot_for_checker(&checker).await;

        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert_eq!(snap.providers.len(), 1);
        assert_eq!(snap.providers[0].provider_id, "counting");
        assert_eq!(snap.providers[0].status, HealthStatus::Degraded);
    }
}
