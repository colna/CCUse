//! T1.0.2.19 — Provider CRUD Tauri commands.
//!
//! `list_providers`, `add_provider`, `update_provider`, `delete_provider`.
//! CRUD commands persist through [`ProviderRepository`] and then hot-
//! reload the runtime [`ProviderManager`]. Errors are stringified at
//! the IPC boundary.

use std::sync::Arc;

use tauri::State;

use crate::providers::anthropic::AnthropicProvider;
use crate::providers::api::{HealthStatus, Provider as _};
use crate::providers::model::{Provider, ProviderInput, ProviderKind};
use crate::providers::openai::OpenAIProvider;
use crate::providers::repository::ProviderRepository;
use crate::providers::{ManagerError, ProviderManager, RepositoryError};

/// Managed state type for the provider repository.
pub type ProviderRepoHandle = Arc<ProviderRepository>;

/// Managed state type for the runtime provider registry.
pub type ProviderManagerHandle = Arc<ProviderManager>;

/// Return all providers (API keys excluded from the model).
#[tauri::command]
pub async fn list_providers(repo: State<'_, ProviderRepoHandle>) -> Result<Vec<Provider>, String> {
    repo.list().map_err(|e| e.to_string())
}

/// Create a new provider and return the persisted row.
#[tauri::command]
pub async fn add_provider(
    repo: State<'_, ProviderRepoHandle>,
    manager: State<'_, ProviderManagerHandle>,
    input: ProviderInput,
) -> Result<Provider, String> {
    add_provider_and_reload(repo.inner().as_ref(), manager.inner().as_ref(), input).await
}

/// Update an existing provider (all fields) and return the refreshed row.
#[tauri::command]
pub async fn update_provider(
    repo: State<'_, ProviderRepoHandle>,
    manager: State<'_, ProviderManagerHandle>,
    id: String,
    input: ProviderInput,
) -> Result<Provider, String> {
    update_provider_and_reload(repo.inner().as_ref(), manager.inner().as_ref(), &id, input).await
}

/// Delete a provider by id.
#[tauri::command]
pub async fn delete_provider(
    repo: State<'_, ProviderRepoHandle>,
    manager: State<'_, ProviderManagerHandle>,
    id: String,
) -> Result<(), String> {
    delete_provider_and_reload(repo.inner().as_ref(), manager.inner().as_ref(), &id).await
}

pub(crate) async fn add_provider_and_reload(
    repo: &ProviderRepository,
    manager: &ProviderManager,
    input: ProviderInput,
) -> Result<Provider, String> {
    let added = repo.add(&input).map_err(|e| e.to_string())?;
    match manager.reload_from_repository(repo).await {
        Ok(_) => Ok(added),
        Err(err) => {
            let rollback = repo.delete(&added.id);
            Err(format_reload_failure("add_provider", &err, rollback))
        }
    }
}

pub(crate) async fn update_provider_and_reload(
    repo: &ProviderRepository,
    manager: &ProviderManager,
    id: &str,
    input: ProviderInput,
) -> Result<Provider, String> {
    let previous = decryptable_provider_input_for_existing_row(repo, id)?;
    let updated = repo.update(id, &input).map_err(|e| e.to_string())?;
    match manager.reload_from_repository(repo).await {
        Ok(_) => Ok(updated),
        Err(err) => {
            if let Some(previous) = previous {
                let rollback = repo.update(id, &previous).map(|_| ());
                Err(format_reload_failure("update_provider", &err, rollback))
            } else {
                Err(format_reload_failure_without_rollback(
                    "update_provider",
                    &err,
                ))
            }
        }
    }
}

pub(crate) async fn delete_provider_and_reload(
    repo: &ProviderRepository,
    manager: &ProviderManager,
    id: &str,
) -> Result<(), String> {
    let previous = decryptable_provider_input_for_existing_row(repo, id)?;
    repo.delete(id).map_err(|e| e.to_string())?;
    match manager.reload_from_repository(repo).await {
        Ok(_) => Ok(()),
        Err(err) => {
            if let Some(previous) = previous {
                let rollback = repo.insert_with_id(id, &previous).map(|_| ());
                Err(format_reload_failure("delete_provider", &err, rollback))
            } else {
                Err(format_reload_failure_without_rollback(
                    "delete_provider",
                    &err,
                ))
            }
        }
    }
}

fn decryptable_provider_input_for_existing_row(
    repo: &ProviderRepository,
    id: &str,
) -> Result<Option<ProviderInput>, String> {
    match provider_input_for_existing_row(repo, id) {
        Ok(input) => Ok(Some(input)),
        Err(RepositoryError::Crypto(err)) => {
            eprintln!(
                "CCUse: provider `{id}` API key cannot be decrypted; allowing overwrite/delete: {err}"
            );
            Ok(None)
        }
        Err(err) => Err(err.to_string()),
    }
}

fn provider_input_for_existing_row(
    repo: &ProviderRepository,
    id: &str,
) -> Result<ProviderInput, RepositoryError> {
    let provider = repo
        .get(id)?
        .ok_or_else(|| RepositoryError::NotFound(id.to_owned()))?;
    let api_key = repo.get_decrypted_api_key(id)?;
    Ok(provider_to_input(provider, api_key))
}

fn provider_to_input(provider: Provider, api_key: String) -> ProviderInput {
    ProviderInput {
        name: provider.name,
        kind: provider.kind,
        base_url: provider.base_url,
        api_key,
        priority: provider.priority,
        enabled: provider.enabled,
        monthly_quota: provider.monthly_quota,
        rate_limit_rpm: provider.rate_limit_rpm,
        cost_per_1k_tokens: provider.cost_per_1k_tokens,
    }
}

fn format_reload_failure(
    action: &str,
    reload_error: &ManagerError,
    rollback: Result<(), RepositoryError>,
) -> String {
    match rollback {
        Ok(()) => {
            format!(
                "{action} failed during provider reload and database rollback succeeded: {reload_error}"
            )
        }
        Err(rollback_error) => {
            format!(
                "{action} failed during provider reload: {reload_error}; database rollback failed: {rollback_error}"
            )
        }
    }
}

fn format_reload_failure_without_rollback(action: &str, reload_error: &ManagerError) -> String {
    format!(
        "{action} failed during provider reload: {reload_error}; database rollback skipped because the previous provider API key could not be decrypted"
    )
}

/// Test connectivity to a provider's endpoint (T1.0.4.05).
///
/// Makes the same lightweight health probe used by the runtime for
/// OpenAI-compatible providers and returns the round-trip time in
/// milliseconds.
#[tauri::command]
pub async fn test_provider_connection(
    repo: State<'_, ProviderRepoHandle>,
    id: String,
) -> Result<u64, String> {
    let provider = repo
        .get(&id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("provider {id} not found"))?;
    let api_key = repo.get_decrypted_api_key(&id).map_err(|e| e.to_string())?;

    let start = std::time::Instant::now();

    if matches!(provider.kind, ProviderKind::Openai | ProviderKind::Relay) {
        let runtime =
            OpenAIProvider::new(&provider.id, &provider.name, &provider.base_url, &api_key)
                .map_err(|e| e.to_string())?;
        let status = runtime.health_check().await.map_err(|e| e.to_string())?;
        if status == HealthStatus::Down {
            return Err("health check reported provider down".to_owned());
        }
        #[allow(clippy::cast_possible_truncation)]
        return Ok(start.elapsed().as_millis() as u64);
    }

    if provider.kind.uses_anthropic_messages() {
        let runtime =
            AnthropicProvider::new(&provider.id, &provider.name, &provider.base_url, &api_key)
                .map_err(|e| e.to_string())?;
        let status = runtime.health_check().await.map_err(|e| e.to_string())?;
        if status == HealthStatus::Down {
            return Err("health check reported provider down".to_owned());
        }
        #[allow(clippy::cast_possible_truncation)]
        return Ok(start.elapsed().as_millis() as u64);
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let url = match provider.kind {
        ProviderKind::Anthropic | ProviderKind::Claude | ProviderKind::Custom => {
            format!("{}/v1/messages", provider.base_url)
        }
        ProviderKind::Gemini => {
            format!("{}/v1beta/models", provider.base_url)
        }
        _ => format!("{}/v1/models", provider.base_url),
    };

    let mut req = client.get(&url);
    match provider.kind {
        ProviderKind::Anthropic | ProviderKind::Claude | ProviderKind::Custom => {
            req = req
                .header("x-api-key", &api_key)
                .header("anthropic-version", "2023-06-01");
        }
        ProviderKind::Gemini => {
            req = req.query(&[("key", &api_key)]);
        }
        _ => {
            req = req.header("Authorization", format!("Bearer {api_key}"));
        }
    }

    let resp = req.send().await.map_err(|e| e.to_string())?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(format!("authentication failed: HTTP {status} {body}"));
        }
        return Err(format!("HTTP {status} {body}"));
    }
    #[allow(clippy::cast_possible_truncation)]
    let ms = start.elapsed().as_millis() as u64;
    Ok(ms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::MasterKey;
    use crate::db::{open_database, run_migrations, Database};
    use crate::providers::model::ProviderKind;
    use rusqlite::params;
    use tempfile::TempDir;

    fn make_repo() -> (TempDir, Arc<ProviderRepository>, Database) {
        let dir = TempDir::new().expect("tempdir");
        let db = open_database(dir.path().join("test.db")).expect("open");
        run_migrations(&db).expect("migrate");
        let key = Arc::new(MasterKey::generate().expect("key"));
        (dir, Arc::new(ProviderRepository::new(db.clone(), key)), db)
    }

    fn sample_input(name: &str, priority: i32) -> ProviderInput {
        ProviderInput {
            name: name.into(),
            kind: ProviderKind::Openai,
            base_url: "https://api.openai.com".into(),
            api_key: "sk-test-key".into(),
            priority,
            enabled: true,
            monthly_quota: None,
            rate_limit_rpm: None,
            cost_per_1k_tokens: None,
        }
    }

    fn corrupt_kind(db: &Database, id: &str) {
        db.with_connection(|conn| {
            conn.execute(
                "UPDATE providers SET kind='future-provider' WHERE id=?1",
                params![id],
            )
        })
        .expect("corrupt kind");
    }

    fn provider_row_count(db: &Database) -> i64 {
        db.with_connection(|conn| {
            conn.query_row("SELECT COUNT(*) FROM providers", [], |row| row.get(0))
        })
        .expect("count providers")
    }

    #[test]
    fn crud_round_trip_via_repository() {
        let (_dir, repo, _db) = make_repo();
        // Add
        let added = repo.add(&sample_input("Test Provider", 50)).expect("add");
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
                    ..sample_input("Test Provider", 50)
                },
            )
            .expect("update");
        assert_eq!(updated.name, "Renamed");

        // Delete
        repo.delete(&added.id).expect("delete");
        assert!(repo.list().expect("list").is_empty());
    }

    #[tokio::test]
    async fn add_provider_and_reload_registers_enabled_provider_immediately() {
        let (_dir, repo, _db) = make_repo();
        let manager = ProviderManager::new();

        let added = add_provider_and_reload(repo.as_ref(), &manager, sample_input("Primary", 10))
            .await
            .expect("add and reload");

        let enabled = manager.enabled_by_priority().await;
        assert_eq!(enabled[0].id(), added.id);
    }

    #[tokio::test]
    async fn update_provider_and_reload_refreshes_runtime_wrapper_immediately() {
        let (_dir, repo, _db) = make_repo();
        let manager = ProviderManager::new();
        let added = add_provider_and_reload(repo.as_ref(), &manager, sample_input("Primary", 20))
            .await
            .expect("add and reload");

        let updated = update_provider_and_reload(
            repo.as_ref(),
            &manager,
            &added.id,
            sample_input("Renamed", 5),
        )
        .await
        .expect("update and reload");

        let wrapper = manager.get(&added.id).await.expect("runtime provider");
        assert_eq!(updated.name, "Renamed");
        assert_eq!(wrapper.name(), "Renamed");
        assert_eq!(wrapper.get_priority(), 5);
    }

    #[tokio::test]
    async fn delete_provider_and_reload_removes_runtime_wrapper_immediately() {
        let (_dir, repo, _db) = make_repo();
        let manager = ProviderManager::new();
        let added = add_provider_and_reload(repo.as_ref(), &manager, sample_input("Primary", 10))
            .await
            .expect("add and reload");

        delete_provider_and_reload(repo.as_ref(), &manager, &added.id)
            .await
            .expect("delete and reload");

        assert!(manager.get(&added.id).await.is_none());
    }

    #[tokio::test]
    async fn add_provider_and_reload_rolls_back_row_when_reload_fails() {
        let (_dir, repo, db) = make_repo();
        let manager = ProviderManager::new();
        let blocker = add_provider_and_reload(repo.as_ref(), &manager, sample_input("Blocker", 10))
            .await
            .expect("add blocker");
        corrupt_kind(&db, &blocker.id);
        let before_count = provider_row_count(&db);

        let err = add_provider_and_reload(repo.as_ref(), &manager, sample_input("New", 20))
            .await
            .expect_err("reload must fail");

        assert!(err.contains("database rollback succeeded"));
        assert_eq!(provider_row_count(&db), before_count);
    }

    #[tokio::test]
    async fn update_provider_and_reload_restores_previous_row_when_reload_fails() {
        let (_dir, repo, db) = make_repo();
        let manager = ProviderManager::new();
        let target = add_provider_and_reload(repo.as_ref(), &manager, sample_input("Primary", 10))
            .await
            .expect("add target");
        let blocker = add_provider_and_reload(repo.as_ref(), &manager, sample_input("Blocker", 20))
            .await
            .expect("add blocker");
        corrupt_kind(&db, &blocker.id);

        let err = update_provider_and_reload(
            repo.as_ref(),
            &manager,
            &target.id,
            sample_input("Renamed", 1),
        )
        .await
        .expect_err("reload must fail");

        assert!(err.contains("database rollback succeeded"));
        let restored = repo
            .get(&target.id)
            .expect("get restored")
            .expect("target still present");
        assert_eq!(restored.name, "Primary");
        assert_eq!(restored.priority, 10);
    }

    #[tokio::test]
    async fn delete_provider_and_reload_restores_deleted_row_when_reload_fails() {
        let (_dir, repo, db) = make_repo();
        let manager = ProviderManager::new();
        let target = add_provider_and_reload(repo.as_ref(), &manager, sample_input("Primary", 10))
            .await
            .expect("add target");
        let blocker = add_provider_and_reload(repo.as_ref(), &manager, sample_input("Blocker", 20))
            .await
            .expect("add blocker");
        corrupt_kind(&db, &blocker.id);

        let err = delete_provider_and_reload(repo.as_ref(), &manager, &target.id)
            .await
            .expect_err("reload must fail");

        assert!(err.contains("database rollback succeeded"));
        assert!(repo.get(&target.id).expect("get restored").is_some());
        assert!(manager.get(&target.id).await.is_some());
    }

    #[tokio::test]
    async fn update_provider_and_reload_repairs_undecryptable_provider() {
        let (_dir, repo, db) = make_repo();
        let added = repo
            .add(&sample_input("Old Key Provider", 10))
            .expect("add with old key");
        let new_key = Arc::new(MasterKey::generate().expect("new key"));
        let new_repo = Arc::new(ProviderRepository::new(db.clone(), new_key));
        let manager = ProviderManager::new();

        let updated = update_provider_and_reload(
            new_repo.as_ref(),
            &manager,
            &added.id,
            sample_input("Repaired Provider", 5),
        )
        .await
        .expect("update should overwrite undecryptable key");

        assert_eq!(updated.name, "Repaired Provider");
        let loaded = manager.get(&added.id).await.expect("runtime loaded");
        assert_eq!(loaded.name(), "Repaired Provider");
    }

    #[tokio::test]
    async fn delete_provider_and_reload_removes_undecryptable_provider() {
        let (_dir, repo, db) = make_repo();
        let added = repo
            .add(&sample_input("Old Key Provider", 10))
            .expect("add with old key");
        let new_key = Arc::new(MasterKey::generate().expect("new key"));
        let new_repo = Arc::new(ProviderRepository::new(db.clone(), new_key));
        let manager = ProviderManager::new();

        delete_provider_and_reload(new_repo.as_ref(), &manager, &added.id)
            .await
            .expect("delete should not require decrypting old key");

        assert_eq!(provider_row_count(&db), 0);
        assert!(manager.is_empty().await);
    }
}
