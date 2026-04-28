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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::ProviderManager;

    #[tokio::test]
    async fn empty_manager_returns_empty_snapshot() {
        let mgr = Arc::new(ProviderManager::new());
        let checker = Arc::new(HealthChecker::new(mgr));
        let snap = checker.snapshot().await;
        assert!(snap.is_empty());
    }
}
