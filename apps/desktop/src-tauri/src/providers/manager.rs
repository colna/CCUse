//! [`ProviderManager`] — global registry of runtime providers.
//!
//! Owns `Vec<Arc<ProviderWrapper>>` and provides lookup, add, remove,
//! and iteration. The `SwitchEngine` (T1.0.2.09+) and `HealthChecker`
//! (T1.0.2.04) both hold a reference to the manager.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use super::api::{Provider as ProviderTrait, ProviderError};
use super::model::ProviderKind;
use super::openai::OpenAIProvider;
use super::repository::{ProviderRepository, RepositoryError};
use super::wrapper::{ProviderWrapper, RuntimeState};

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
        let wrappers = build_wrappers_from_repository(repo, &HashMap::new())?;
        let count = wrappers.len();
        *self.providers.write().await = wrappers;
        Ok(count)
    }

    /// Reload providers from the repository while preserving runtime
    /// state for entries whose id still exists. The database read and
    /// provider construction happen before the write lock, so a failed
    /// reload leaves the current registry untouched.
    pub async fn reload_from_repository(
        &self,
        repo: &ProviderRepository,
    ) -> Result<usize, ManagerError> {
        let existing_states = {
            let guard = self.providers.read().await;
            guard
                .iter()
                .map(|wrapper| (wrapper.id().to_owned(), wrapper.runtime_state()))
                .collect::<HashMap<_, _>>()
        };

        let wrappers = build_wrappers_from_repository(repo, &existing_states)?;
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

fn build_wrappers_from_repository(
    repo: &ProviderRepository,
    existing_states: &HashMap<String, Arc<RuntimeState>>,
) -> Result<Vec<Arc<ProviderWrapper>>, ManagerError> {
    let db_providers = repo.list()?;
    let mut wrappers = Vec::with_capacity(db_providers.len());
    for p in &db_providers {
        let api_key = match repo.get_decrypted_api_key(&p.id) {
            Ok(api_key) => api_key,
            Err(RepositoryError::Crypto(err)) => {
                eprintln!(
                    "CCUse: skipping provider `{}` because its API key cannot be decrypted: {err}",
                    p.id,
                );
                continue;
            }
            Err(err) => return Err(ManagerError::Repository(err)),
        };
        let inner = build_runtime_provider(&p.id, &p.name, p.kind, &p.base_url, &api_key)?;
        let state = existing_states
            .get(&p.id)
            .cloned()
            .unwrap_or_else(|| Arc::new(RuntimeState::new()));
        wrappers.push(Arc::new(ProviderWrapper::new_with_state(
            &p.id, &p.name, p.kind, p.priority, None, // cost_per_token — not persisted yet
            p.enabled, inner, state,
        )));
    }
    Ok(wrappers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::MasterKey;
    use crate::db::{open_database, run_migrations, Database};
    use crate::providers::api::{HealthStatus, Provider as RuntimeProviderTrait};
    use crate::providers::model::ProviderInput;
    use tempfile::TempDir;

    fn fresh_repo() -> (TempDir, ProviderRepository) {
        let (dir, _db, repo) = fresh_repo_with_db();
        (dir, repo)
    }

    fn fresh_repo_with_db() -> (TempDir, Database, ProviderRepository) {
        let dir = TempDir::new().expect("tempdir");
        let db = open_database(dir.path().join("ccuse.db")).expect("open ok");
        run_migrations(&db).expect("migrate ok");
        let key = Arc::new(MasterKey::generate().expect("rng"));
        (dir, db.clone(), ProviderRepository::new(db, key))
    }

    fn sample_input() -> ProviderInput {
        ProviderInput {
            name: "Work OpenAI".to_owned(),
            kind: ProviderKind::Openai,
            base_url: "https://api.openai.com".to_owned(),
            api_key: "sk-real-secret-1234".to_owned(),
            priority: 50,
            enabled: true,
            monthly_quota: None,
            rate_limit_rpm: None,
            cost_per_1k_tokens: None,
        }
    }

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

    #[tokio::test]
    async fn reload_from_repository_preserves_runtime_state_for_existing_provider() {
        let (_dir, repo) = fresh_repo();
        let saved = repo.add(&sample_input()).expect("add provider");
        let mgr = ProviderManager::new();
        mgr.load_from_repository(&repo).await.expect("initial load");
        let before = mgr.get(&saved.id).await.expect("loaded provider");
        before.state.set_health(HealthStatus::Degraded).await;

        let updated = ProviderInput {
            name: "Renamed OpenAI".into(),
            priority: 5,
            ..sample_input()
        };
        repo.update(&saved.id, &updated).expect("update provider");

        let count = mgr
            .reload_from_repository(&repo)
            .await
            .expect("reload providers");

        assert_eq!(count, 1);
        let after = mgr.get(&saved.id).await.expect("reloaded provider");
        assert_eq!(after.name(), "Renamed OpenAI");
        assert_eq!(after.get_priority(), 5);
        assert_eq!(after.state.health().await, HealthStatus::Degraded);
    }

    #[tokio::test]
    async fn load_from_repository_skips_provider_with_undecryptable_api_key() {
        let (_dir, db, repo) = fresh_repo_with_db();
        repo.add(&sample_input()).expect("add provider");
        let wrong_key = Arc::new(MasterKey::generate().expect("wrong key"));
        let wrong_repo = ProviderRepository::new(db, wrong_key);
        let mgr = ProviderManager::new();

        let count = mgr
            .load_from_repository(&wrong_repo)
            .await
            .expect("load should skip bad ciphertext");

        assert_eq!(count, 0);
        assert!(mgr.is_empty().await);
        assert_eq!(
            wrong_repo.list().expect("metadata remains visible").len(),
            1
        );
    }
}
