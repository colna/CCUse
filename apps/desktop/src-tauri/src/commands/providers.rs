//! T1.0.2.19 — Provider CRUD Tauri commands.
//!
//! `list_providers`, `add_provider`, `update_provider`, `delete_provider`.
//! Thin wrappers over [`ProviderRepository`]; errors are stringified at
//! the IPC boundary.

use std::sync::Arc;

use tauri::State;

use crate::providers::model::{Provider, ProviderInput};
use crate::providers::repository::ProviderRepository;

/// Managed state type for the provider repository.
pub type ProviderRepoHandle = Arc<ProviderRepository>;

/// Return all providers (API keys excluded from the model).
#[tauri::command]
pub async fn list_providers(
    repo: State<'_, ProviderRepoHandle>,
) -> Result<Vec<Provider>, String> {
    repo.list().map_err(|e| e.to_string())
}

/// Create a new provider and return the persisted row.
#[tauri::command]
pub async fn add_provider(
    repo: State<'_, ProviderRepoHandle>,
    input: ProviderInput,
) -> Result<Provider, String> {
    repo.add(&input).map_err(|e| e.to_string())
}

/// Update an existing provider (all fields) and return the refreshed row.
#[tauri::command]
pub async fn update_provider(
    repo: State<'_, ProviderRepoHandle>,
    id: String,
    input: ProviderInput,
) -> Result<Provider, String> {
    repo.update(&id, &input).map_err(|e| e.to_string())
}

/// Delete a provider by id.
#[tauri::command]
pub async fn delete_provider(
    repo: State<'_, ProviderRepoHandle>,
    id: String,
) -> Result<(), String> {
    repo.delete(&id).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::MasterKey;
    use crate::db::{open_database, run_migrations};
    use crate::providers::model::ProviderKind;
    use tempfile::TempDir;

    fn make_repo() -> (TempDir, Arc<ProviderRepository>) {
        let dir = TempDir::new().expect("tempdir");
        let db = open_database(dir.path().join("test.db")).expect("open");
        run_migrations(&db).expect("migrate");
        let key = Arc::new(MasterKey::generate().expect("key"));
        (dir, Arc::new(ProviderRepository::new(db, key)))
    }

    fn sample_input() -> ProviderInput {
        ProviderInput {
            name: "Test Provider".into(),
            kind: ProviderKind::Openai,
            base_url: "https://api.openai.com".into(),
            api_key: "sk-test-key".into(),
            priority: 50,
            enabled: true,
        }
    }

    #[test]
    fn crud_round_trip_via_repository() {
        let (_dir, repo) = make_repo();
        // Add
        let added = repo.add(&sample_input()).expect("add");
        assert_eq!(added.name, "Test Provider");

        // List
        let all = repo.list().expect("list");
        assert_eq!(all.len(), 1);

        // Update
        let updated = repo
            .update(
                &added.id,
                &ProviderInput {
                    name: "Renamed".into(),
                    ..sample_input()
                },
            )
            .expect("update");
        assert_eq!(updated.name, "Renamed");

        // Delete
        repo.delete(&added.id).expect("delete");
        assert!(repo.list().expect("list").is_empty());
    }
}
