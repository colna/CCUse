//! [`ProviderManager`] — global registry of runtime providers.
//!
//! Owns `Vec<Arc<ProviderWrapper>>` and provides lookup, add, remove,
//! and iteration. The `SwitchEngine` (T1.0.2.09+) and `HealthChecker`
//! (T1.0.2.04) both hold a reference to the manager.

use std::sync::Arc;

use tokio::sync::RwLock;

use super::api::{Provider as ProviderTrait, ProviderError};
use super::model::ProviderKind;
use super::openai::OpenAIProvider;
use super::repository::ProviderRepository;
use super::wrapper::ProviderWrapper;

/// Errors specific to [`ProviderManager`] operations.
#[derive(thiserror::Error, Debug)]
pub enum ManagerError {
    #[error("provider `{0}` not found")]
    NotFound(String),
    #[error("provider `{0}` already registered")]
    AlreadyExists(String),
    #[error(transparent)]
    Repository(#[from] super::repository::RepositoryError),
    #[error(transparent)]
    Provider(#[from] ProviderError),
}

/// Thread-safe provider registry. All mutations go through async
/// methods that acquire a write lock; reads (listing, lookup) use a
/// read lock and are non-blocking when no write is in progress.
#[derive(Debug)]
pub struct ProviderManager {
    providers: RwLock<Vec<Arc<ProviderWrapper>>>,
}

impl ProviderManager {
    /// Create an empty manager. Call [`Self::load_from_repository`]
    /// after construction to hydrate from the database.
    #[must_use]
    pub fn new() -> Self {
        Self {
            providers: RwLock::new(Vec::new()),
        }
    }

    /// Hydrate the registry from the database. Existing entries are
    /// replaced wholesale — call this at app startup or after a bulk
    /// import.
    pub async fn load_from_repository(
        &self,
        repo: &ProviderRepository,
    ) -> Result<usize, ManagerError> {
        let db_providers = repo.list()?;
        let mut wrappers = Vec::with_capacity(db_providers.len());
        for p in &db_providers {
            let api_key = repo.get_decrypted_api_key(&p.id)?;
            let inner = build_runtime_provider(&p.id, &p.name, p.kind, &p.base_url, &api_key)?;
            wrappers.push(Arc::new(ProviderWrapper::new(
                &p.id, &p.name, p.kind, p.priority,
                None, // cost_per_token — not persisted yet
                p.enabled, inner,
            )));
        }
        let count = wrappers.len();
        *self.providers.write().await = wrappers;
        Ok(count)
    }

    /// Register a new wrapper. Errors if a provider with the same id
    /// is already present.
    pub async fn add(&self, wrapper: Arc<ProviderWrapper>) -> Result<(), ManagerError> {
        let mut guard = self.providers.write().await;
        if guard.iter().any(|w| w.id() == wrapper.id()) {
            return Err(ManagerError::AlreadyExists(wrapper.id().to_owned()));
        }
        guard.push(wrapper);
        Ok(())
    }

    /// Remove a provider by id. Returns the removed wrapper (useful
    /// for teardown / logging). Errors if not found.
    pub async fn remove(&self, id: &str) -> Result<Arc<ProviderWrapper>, ManagerError> {
        let mut guard = self.providers.write().await;
        let idx = guard
            .iter()
            .position(|w| w.id() == id)
            .ok_or_else(|| ManagerError::NotFound(id.to_owned()))?;
        Ok(guard.swap_remove(idx))
    }

    /// Look up a single provider by id.
    pub async fn get(&self, id: &str) -> Option<Arc<ProviderWrapper>> {
        self.providers
            .read()
            .await
            .iter()
            .find(|w| w.id() == id)
            .cloned()
    }

    /// Snapshot of all registered providers (cheap `Arc` clones).
    pub async fn list(&self) -> Vec<Arc<ProviderWrapper>> {
        self.providers.read().await.clone()
    }

    /// Only enabled providers, sorted by priority ascending (lower =
    /// preferred). This is the input to all switch strategies.
    pub async fn enabled_by_priority(&self) -> Vec<Arc<ProviderWrapper>> {
        let guard = self.providers.read().await;
        let mut result: Vec<_> = guard.iter().filter(|w| w.is_enabled()).cloned().collect();
        result.sort_by_key(|w| w.get_priority());
        result
    }

    /// Number of registered providers.
    pub async fn len(&self) -> usize {
        self.providers.read().await.len()
    }

    /// Whether the registry is empty.
    pub async fn is_empty(&self) -> bool {
        self.providers.read().await.is_empty()
    }
}

impl Default for ProviderManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a runtime provider from config. Today only `OpenAI`-
/// compatible providers exist; `Anthropic` / `Gemini` land in T1.0.3.
fn build_runtime_provider(
    id: &str,
    name: &str,
    kind: ProviderKind,
    base_url: &str,
    api_key: &str,
) -> Result<Box<dyn super::api::Provider>, ProviderError> {
    match kind {
        ProviderKind::Openai | ProviderKind::Custom => {
            Ok(Box::new(OpenAIProvider::new(id, name, base_url, api_key)?))
        }
        ProviderKind::Anthropic | ProviderKind::Gemini | ProviderKind::Relay => {
            // T1.0.3 will add AnthropicProvider / GeminiProvider.
            // Until then, fall back to the OpenAI-compatible path —
            // users who add these kinds will get a best-effort proxy
            // that works if the upstream accepts OpenAI-shaped requests.
            Ok(Box::new(OpenAIProvider::new(id, name, base_url, api_key)?))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::api::Provider as RuntimeProviderTrait;

    /// Build a wrapper from mock data (no DB, no real HTTP client).
    fn mock_wrapper(id: &str, priority: i32, enabled: bool) -> Arc<ProviderWrapper> {
        let inner = OpenAIProvider::new(id, id, "https://mock.api", "sk-test").expect("build");
        Arc::new(ProviderWrapper::new(
            id,
            id,
            ProviderKind::Openai,
            priority,
            None,
            enabled,
            Box::new(inner),
        ))
    }

    #[tokio::test]
    async fn new_manager_is_empty() {
        let mgr = ProviderManager::new();
        assert!(mgr.is_empty().await);
        assert_eq!(mgr.len().await, 0);
    }

    #[tokio::test]
    async fn add_and_get_round_trip() {
        let mgr = ProviderManager::new();
        let w = mock_wrapper("p1", 10, true);
        mgr.add(w).await.expect("add ok");
        assert_eq!(mgr.len().await, 1);
        let found = mgr.get("p1").await.expect("found");
        assert_eq!(found.id(), "p1");
    }

    #[tokio::test]
    async fn add_duplicate_id_errors() {
        let mgr = ProviderManager::new();
        mgr.add(mock_wrapper("p1", 10, true)).await.expect("ok");
        let err = mgr
            .add(mock_wrapper("p1", 20, true))
            .await
            .expect_err("must fail");
        assert!(matches!(err, ManagerError::AlreadyExists(_)));
    }

    #[tokio::test]
    async fn remove_returns_wrapper_and_decrements_len() {
        let mgr = ProviderManager::new();
        mgr.add(mock_wrapper("p1", 10, true)).await.unwrap();
        mgr.add(mock_wrapper("p2", 20, true)).await.unwrap();
        assert_eq!(mgr.len().await, 2);
        let removed = mgr.remove("p1").await.expect("remove ok");
        assert_eq!(removed.id(), "p1");
        assert_eq!(mgr.len().await, 1);
        assert!(mgr.get("p1").await.is_none());
    }

    #[tokio::test]
    async fn remove_unknown_id_errors() {
        let mgr = ProviderManager::new();
        let err = mgr.remove("ghost").await.expect_err("must fail");
        assert!(matches!(err, ManagerError::NotFound(_)));
    }

    #[tokio::test]
    async fn enabled_by_priority_filters_and_sorts() {
        let mgr = ProviderManager::new();
        mgr.add(mock_wrapper("a", 50, true)).await.unwrap();
        mgr.add(mock_wrapper("b", 10, true)).await.unwrap();
        mgr.add(mock_wrapper("c", 30, false)).await.unwrap();
        mgr.add(mock_wrapper("d", 5, true)).await.unwrap();

        let result = mgr.enabled_by_priority().await;
        let ids: Vec<&str> = result.iter().map(|w| w.id()).collect();
        assert_eq!(ids, vec!["d", "b", "a"]);
    }

    #[tokio::test]
    async fn list_returns_all_including_disabled() {
        let mgr = ProviderManager::new();
        mgr.add(mock_wrapper("a", 10, true)).await.unwrap();
        mgr.add(mock_wrapper("b", 20, false)).await.unwrap();
        assert_eq!(mgr.list().await.len(), 2);
    }

    #[tokio::test]
    async fn get_unknown_returns_none() {
        let mgr = ProviderManager::new();
        assert!(mgr.get("ghost").await.is_none());
    }

    #[test]
    fn build_runtime_provider_creates_openai_for_all_kinds() {
        for kind in [
            ProviderKind::Openai,
            ProviderKind::Anthropic,
            ProviderKind::Gemini,
            ProviderKind::Custom,
        ] {
            let p = build_runtime_provider("id", "n", kind, "https://api", "sk-key");
            assert!(p.is_ok(), "kind {kind:?} must build successfully");
        }
    }
}
