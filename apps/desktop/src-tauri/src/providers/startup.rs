//! Boot-time helpers for the provider stack.
//!
//! Lives outside [`ProviderManager`] so the wiring used by
//! `lib.rs::setup` (T1.0.6.04) can be unit-tested with a real
//! `ProviderRepository` without spinning up Tauri.

use super::{ManagerError, ProviderManager, ProviderRepository};

/// Hydrate `manager` from `repo` at app boot.
///
/// Failure is intentionally non-fatal: the desktop app must still
/// come up so the user can fix the bad row through the UI. We log
/// to `stderr` (panic hook captures it) and return `0` so the
/// caller can continue.
pub async fn load_initial_providers(manager: &ProviderManager, repo: &ProviderRepository) -> usize {
    match manager.load_from_repository(repo).await {
        Ok(count) => count,
        Err(err) => {
            eprintln!("CCUse: provider hydration failed at startup: {err}");
            let _: ManagerError = err;
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tempfile::TempDir;

    use super::*;
    use crate::crypto::MasterKey;
    use crate::db::{open_database, run_migrations};
    use crate::providers::{ProviderInput, ProviderKind};

    fn fresh_repo() -> (TempDir, ProviderRepository) {
        let dir = TempDir::new().expect("tempdir");
        let db = open_database(dir.path().join("ccuse.db")).expect("open ok");
        run_migrations(&db).expect("migrate ok");
        let key = Arc::new(MasterKey::generate().expect("rng"));
        (dir, ProviderRepository::new(db, key))
    }

    fn sample_input(name: &str, priority: i32) -> ProviderInput {
        ProviderInput {
            name: name.to_owned(),
            kind: ProviderKind::Openai,
            base_url: "https://api.openai.com".to_owned(),
            api_key: "sk-test-1234".to_owned(),
            priority,
            enabled: true,
            monthly_quota: None,
            rate_limit_rpm: None,
            cost_per_1k_tokens: None,
        }
    }

    #[tokio::test]
    async fn load_initial_providers_with_empty_repo_returns_zero() {
        let (_dir, repo) = fresh_repo();
        let manager = ProviderManager::new();

        let loaded = load_initial_providers(&manager, &repo).await;

        assert_eq!(loaded, 0);
        assert!(manager.is_empty().await);
    }

    #[tokio::test]
    async fn load_initial_providers_hydrates_manager_from_repo() {
        let (_dir, repo) = fresh_repo();
        repo.add(&sample_input("Primary", 10)).expect("add ok");
        repo.add(&sample_input("Backup", 20)).expect("add ok");
        let manager = ProviderManager::new();

        let loaded = load_initial_providers(&manager, &repo).await;

        assert_eq!(loaded, 2);
        assert_eq!(manager.len().await, 2);
    }
}
