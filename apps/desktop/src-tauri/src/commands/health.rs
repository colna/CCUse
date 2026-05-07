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

/// Run a health probe immediately and return the refreshed snapshot.
#[tauri::command]
pub async fn refresh_health_snapshot(
    checker: State<'_, HealthCheckerHandle>,
) -> Result<HealthSnapshotResponse, String> {
    Ok(refresh_health_snapshot_for_checker(checker.inner().as_ref()).await)
}

pub(crate) async fn refresh_health_snapshot_for_checker(
    checker: &HealthChecker,
) -> HealthSnapshotResponse {
    checker.probe_once().await;
    let providers = checker.snapshot().await;
    HealthSnapshotResponse { providers }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::api::{
        ApiRequest, ApiResponse, HealthStatus, Provider as ProviderTrait, ProviderError,
        StreamingResponse,
    };
    use crate::providers::{ProviderKind, ProviderManager, ProviderWrapper};
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[derive(Debug)]
    struct CountingProvider {
        id: String,
        probes: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl ProviderTrait for CountingProvider {
        fn id(&self) -> &str {
            &self.id
        }

        fn name(&self) -> &str {
            &self.id
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
            self.probes.fetch_add(1, Ordering::Relaxed);
            Ok(HealthStatus::Healthy)
        }

        async fn send_request(&self, _: ApiRequest) -> Result<ApiResponse, ProviderError> {
            Err(ProviderError::BadRequest("unused".into()))
        }

        async fn send_stream_request(
            &self,
            _: ApiRequest,
        ) -> Result<StreamingResponse, ProviderError> {
            Err(ProviderError::BadRequest("unused".into()))
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
    async fn refresh_probe_updates_snapshot_cache() {
        let mgr = Arc::new(ProviderManager::new());
        let probes = Arc::new(AtomicUsize::new(0));
        mgr.add(Arc::new(ProviderWrapper::new(
            "provider-1",
            "Provider 1",
            ProviderKind::Openai,
            10,
            None,
            true,
            Box::new(CountingProvider {
                id: "provider-1".to_owned(),
                probes: Arc::clone(&probes),
            }),
        )))
        .await
        .expect("add provider");
        let checker = HealthChecker::new(mgr);

        let response = refresh_health_snapshot_for_checker(&checker).await;

        assert_eq!(probes.load(Ordering::Relaxed), 1);
        assert_eq!(response.providers[0].provider_id, "provider-1");
        assert_eq!(response.providers[0].status, HealthStatus::Healthy);
    }
}
