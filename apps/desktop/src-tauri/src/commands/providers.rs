//! T1.0.2.19 — Provider CRUD Tauri commands.
//!
//! `list_providers`, `add_provider`, `update_provider`, `delete_provider`.
//! CRUD commands persist through [`ProviderRepository`] and then hot-
//! reload the runtime [`ProviderManager`]. Errors are stringified at
//! the IPC boundary.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use futures::StreamExt;
use tauri::State;

use crate::health::HealthChecker;
use crate::providers::model::{Provider, ProviderInput};
use crate::providers::repository::ProviderRepository;
use crate::providers::{format_reqwest_error, ManagerError, ProviderManager, RepositoryError};
use serde::{Deserialize, Serialize};

const PROVIDER_CONNECTION_TEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StreamCheckStatus {
    Operational,
    Degraded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamCheckResult {
    pub status: StreamCheckStatus,
    pub success: bool,
    pub message: String,
    pub response_time_ms: Option<u64>,
    pub http_status: Option<u16>,
    pub model_used: String,
    pub tested_at: i64,
    pub retry_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_category: Option<String>,
}

/// Managed state type for the provider repository.
pub type ProviderRepoHandle = Arc<ProviderRepository>;

/// Managed state type for the runtime provider registry.
pub type ProviderManagerHandle = Arc<ProviderManager>;

/// Managed state type for health snapshots.
pub type HealthCheckerHandle = Arc<HealthChecker>;

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
    checker: State<'_, HealthCheckerHandle>,
    input: ProviderInput,
) -> Result<Provider, String> {
    let provider =
        add_provider_and_reload(repo.inner().as_ref(), manager.inner().as_ref(), input).await?;
    checker.probe_once().await;
    Ok(provider)
}

/// Update an existing provider (all fields) and return the refreshed row.
#[tauri::command]
pub async fn update_provider(
    repo: State<'_, ProviderRepoHandle>,
    manager: State<'_, ProviderManagerHandle>,
    checker: State<'_, HealthCheckerHandle>,
    id: String,
    input: ProviderInput,
) -> Result<Provider, String> {
    let provider =
        update_provider_and_reload(repo.inner().as_ref(), manager.inner().as_ref(), &id, input)
            .await?;
    checker.probe_once().await;
    Ok(provider)
}

/// Delete a provider by id.
#[tauri::command]
pub async fn delete_provider(
    repo: State<'_, ProviderRepoHandle>,
    manager: State<'_, ProviderManagerHandle>,
    checker: State<'_, HealthCheckerHandle>,
    id: String,
) -> Result<(), String> {
    delete_provider_and_reload(repo.inner().as_ref(), manager.inner().as_ref(), &id).await?;
    checker.probe_once().await;
    Ok(())
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
    let previous = maybe_provider_input_for_existing_row(repo, id).map_err(|e| e.to_string())?;
    let input = merge_existing_api_key_for_update(input, previous.as_ref())?;
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
                    "previous api key could not be decrypted",
                ))
            }
        }
    }
}

fn merge_existing_api_key_for_update(
    mut input: ProviderInput,
    previous: Option<&ProviderInput>,
) -> Result<ProviderInput, String> {
    if input.api_key.trim().is_empty() {
        let previous = previous.ok_or_else(|| {
            "update_provider requires a replacement api key because the previous api key could not be decrypted".to_owned()
        })?;
        input.api_key.clone_from(&previous.api_key);
    }
    Ok(input)
}

pub(crate) async fn delete_provider_and_reload(
    repo: &ProviderRepository,
    manager: &ProviderManager,
    id: &str,
) -> Result<(), String> {
    let previous = maybe_provider_input_for_existing_row(repo, id).map_err(|e| e.to_string())?;
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
                    "previous api key could not be decrypted",
                ))
            }
        }
    }
}

fn maybe_provider_input_for_existing_row(
    repo: &ProviderRepository,
    id: &str,
) -> Result<Option<ProviderInput>, RepositoryError> {
    let provider = repo
        .get(id)?
        .ok_or_else(|| RepositoryError::NotFound(id.to_owned()))?;
    let api_key = match repo.get_decrypted_api_key(id) {
        Ok(api_key) => api_key,
        Err(RepositoryError::Crypto(err)) => {
            eprintln!(
                "CCUse: provider `{id}` api key cannot be decrypted; allowing replacement/delete: {err}",
            );
            return Ok(None);
        }
        Err(err) => return Err(err),
    };
    Ok(Some(provider_to_input(provider, api_key)))
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

fn format_reload_failure_without_rollback(
    action: &str,
    reload_error: &ManagerError,
    reason: &str,
) -> String {
    format!("{action} failed during provider reload and database rollback was unavailable ({reason}): {reload_error}")
}

/// Test connectivity to a provider's endpoint (T1.0.4.05).
///
/// Sends a stream probe that mirrors the provider's real request shape
/// and returns a structured result instead of a bare latency.
#[tauri::command]
pub async fn test_provider_connection(
    repo: State<'_, ProviderRepoHandle>,
    id: String,
) -> Result<StreamCheckResult, String> {
    test_provider_connection_with_repo(repo.inner().as_ref(), &id).await
}

pub(crate) async fn test_provider_connection_with_repo(
    repo: &ProviderRepository,
    id: &str,
) -> Result<StreamCheckResult, String> {
    let provider = repo
        .get(id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("provider {id} not found"))?;
    let api_key = repo.get_decrypted_api_key(id).map_err(|e| e.to_string())?;

    let start = std::time::Instant::now();
    let client = reqwest::Client::builder()
        .timeout(PROVIDER_CONNECTION_TEST_TIMEOUT)
        .build()
        .map_err(format_reqwest_error)?;

    let model_used = if matches!(
        provider.kind,
        crate::providers::model::ProviderKind::Anthropic
    ) {
        "claude-3-5-sonnet-20241022"
    } else {
        "gpt-4o-mini"
    }
    .to_string();

    let request = match provider.kind {
        crate::providers::model::ProviderKind::Anthropic => client
            .post(format!(
                "{}/v1/messages",
                provider.base_url.trim_end_matches('/')
            ))
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .header("accept", "text/event-stream")
            .header("accept-encoding", "identity")
            .json(&anthropic_probe_body(&model_used)),
        crate::providers::model::ProviderKind::Openai
        | crate::providers::model::ProviderKind::Custom
        | crate::providers::model::ProviderKind::Relay => client
            .post(format!(
                "{}/v1/chat/completions",
                provider.base_url.trim_end_matches('/')
            ))
            .header("Authorization", format!("Bearer {api_key}"))
            .header("content-type", "application/json")
            .header("accept", "text/event-stream")
            .header("accept-encoding", "identity")
            .json(&openai_probe_body(&model_used)),
        crate::providers::model::ProviderKind::Gemini => client
            .post(format!(
                "{}/v1beta/models/{model_used}:streamGenerateContent",
                provider.base_url.trim_end_matches('/')
            ))
            .query(&[("key", &api_key)])
            .header("content-type", "application/json")
            .header("accept", "text/event-stream")
            .header("accept-encoding", "identity")
            .json(&gemini_probe_body()),
    };

    match request.send().await {
        Ok(resp) => build_stream_check_result(resp, start, &model_used).await,
        Err(err) => Ok(request_error_result(&err, start, &model_used)),
    }
}

async fn build_stream_check_result(
    resp: reqwest::Response,
    start: std::time::Instant,
    model_used: &str,
) -> Result<StreamCheckResult, String> {
    let tested_at = unix_timestamp_now();
    let http_status = resp.status().as_u16();
    if !resp.status().is_success() {
        let body_text = resp.text().await.map_err(format_reqwest_error)?;
        let body_text = body_text.trim();
        let error_category =
            detect_error_category(http_status, body_text).map(std::string::ToString::to_string);
        #[allow(clippy::cast_possible_truncation)]
        let response_time_ms = Some(start.elapsed().as_millis() as u64);
        return Ok(StreamCheckResult {
            status: StreamCheckStatus::Failed,
            success: false,
            message: classify_http_status(http_status).to_string(),
            response_time_ms,
            http_status: Some(http_status),
            model_used: model_used.to_string(),
            tested_at,
            retry_count: 0,
            error_category,
        });
    }

    let mut stream = resp.bytes_stream();
    let first_chunk = stream.next().await;
    #[allow(clippy::cast_possible_truncation)]
    let response_time_ms_value = start.elapsed().as_millis() as u64;
    let response_time_ms = Some(response_time_ms_value);

    match first_chunk {
        Some(Ok(_)) => {
            let stream_status = if response_time_ms_value > 6000 {
                StreamCheckStatus::Degraded
            } else {
                StreamCheckStatus::Operational
            };
            Ok(StreamCheckResult {
                status: stream_status,
                success: true,
                message: "Check succeeded".to_string(),
                response_time_ms,
                http_status: Some(http_status),
                model_used: model_used.to_string(),
                tested_at,
                retry_count: 0,
                error_category: None,
            })
        }
        Some(Err(err)) => Ok(StreamCheckResult {
            status: StreamCheckStatus::Failed,
            success: false,
            message: err.to_string(),
            response_time_ms,
            http_status: Some(http_status),
            model_used: model_used.to_string(),
            tested_at,
            retry_count: 0,
            error_category: None,
        }),
        None => Ok(StreamCheckResult {
            status: StreamCheckStatus::Failed,
            success: false,
            message: "No response data received".to_string(),
            response_time_ms,
            http_status: Some(http_status),
            model_used: model_used.to_string(),
            tested_at,
            retry_count: 0,
            error_category: None,
        }),
    }
}

fn request_error_result(
    err: &reqwest::Error,
    start: std::time::Instant,
    model_used: &str,
) -> StreamCheckResult {
    let message = if err.is_timeout() {
        "Request timeout"
    } else if err.is_connect() {
        "Connection failed"
    } else {
        "Request failed"
    };

    #[allow(clippy::cast_possible_truncation)]
    let response_time_ms = Some(start.elapsed().as_millis() as u64);

    StreamCheckResult {
        status: StreamCheckStatus::Failed,
        success: false,
        message: format!("{message}: {err}"),
        response_time_ms,
        http_status: None,
        model_used: model_used.to_string(),
        tested_at: unix_timestamp_now(),
        retry_count: 0,
        error_category: None,
    }
}

fn unix_timestamp_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}

fn anthropic_probe_body(model: &str) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "max_tokens": 1,
        "messages": [{"role": "user", "content": "ping"}],
        "stream": true
    })
}

fn openai_probe_body(model: &str) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": "ping"}],
        "max_tokens": 1,
        "stream": true
    })
}

fn gemini_probe_body() -> serde_json::Value {
    serde_json::json!({
        "contents": [{
            "role": "user",
            "parts": [{"text": "ping"}]
        }],
        "generationConfig": {
            "maxOutputTokens": 1
        }
    })
}

fn detect_error_category(status: u16, body: &str) -> Option<&'static str> {
    if !(400..500).contains(&status) {
        return None;
    }
    let lower = body.to_lowercase();
    if !lower.contains("model") {
        return None;
    }
    let indicators = [
        "model_not_found",
        "model not found",
        "does not exist",
        "invalid_model",
        "invalid model",
        "unknown_model",
        "unknown model",
        "is not a valid model",
        "not_found_error",
    ];
    if indicators.iter().any(|s| lower.contains(s)) {
        Some("modelNotFound")
    } else {
        None
    }
}

fn classify_http_status(status: u16) -> &'static str {
    match status {
        400 => "Bad request (400)",
        401 => "Auth rejected (401)",
        402 => "Payment required (402)",
        403 => "Access denied (403)",
        404 => "Not found (404)",
        429 => "Rate limited (429)",
        500 => "Internal server error (500)",
        502 => "Bad gateway (502)",
        503 => "Service unavailable (503)",
        504 => "Gateway timeout (504)",
        s if (500..600).contains(&s) => "Server error",
        _ => "HTTP error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::MasterKey;
    use crate::db::{open_database, run_migrations, Database};
    use crate::providers::api::Provider as _;
    use crate::providers::model::ProviderKind;
    use rusqlite::params;
    use tempfile::TempDir;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

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

    fn corrupt_api_key(db: &Database, id: &str) {
        db.with_connection(|conn| {
            conn.execute(
                "UPDATE providers SET encrypted_api_key=?1 WHERE id=?2",
                params![vec![1_u8, 2, 3], id],
            )
        })
        .expect("corrupt api key");
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
    async fn update_provider_and_reload_keeps_existing_api_key_when_input_key_is_blank() {
        let (_dir, repo, _db) = make_repo();
        let manager = ProviderManager::new();
        let added = add_provider_and_reload(
            repo.as_ref(),
            &manager,
            ProviderInput {
                api_key: "sk-original-key".into(),
                ..sample_input("Primary", 20)
            },
        )
        .await
        .expect("add and reload");

        update_provider_and_reload(
            repo.as_ref(),
            &manager,
            &added.id,
            ProviderInput {
                api_key: String::new(),
                ..sample_input("Renamed", 5)
            },
        )
        .await
        .expect("update keeps api key");

        let key = repo
            .get_decrypted_api_key(&added.id)
            .expect("decrypt api key");
        assert_eq!(key, "sk-original-key");
    }

    #[tokio::test]
    async fn update_provider_and_reload_replaces_api_key_when_input_key_is_present() {
        let (_dir, repo, _db) = make_repo();
        let manager = ProviderManager::new();
        let added = add_provider_and_reload(
            repo.as_ref(),
            &manager,
            ProviderInput {
                api_key: "sk-original-key".into(),
                ..sample_input("Primary", 20)
            },
        )
        .await
        .expect("add and reload");

        update_provider_and_reload(
            repo.as_ref(),
            &manager,
            &added.id,
            ProviderInput {
                api_key: "sk-replacement-key".into(),
                ..sample_input("Renamed", 5)
            },
        )
        .await
        .expect("update replaces api key");

        let key = repo
            .get_decrypted_api_key(&added.id)
            .expect("decrypt api key");
        assert_eq!(key, "sk-replacement-key");
    }

    #[tokio::test]
    async fn update_provider_and_reload_replaces_unreadable_api_key() {
        let (_dir, repo, db) = make_repo();
        let manager = ProviderManager::new();
        let added = add_provider_and_reload(repo.as_ref(), &manager, sample_input("Primary", 20))
            .await
            .expect("add and reload");
        corrupt_api_key(&db, &added.id);

        let updated = update_provider_and_reload(
            repo.as_ref(),
            &manager,
            &added.id,
            ProviderInput {
                api_key: "sk-replacement-key".into(),
                ..sample_input("Renamed", 5)
            },
        )
        .await
        .expect("update replaces unreadable key");

        let wrapper = manager.get(&added.id).await.expect("runtime provider");
        assert_eq!(updated.name, "Renamed");
        assert_eq!(wrapper.name(), "Renamed");
        assert_eq!(wrapper.get_priority(), 5);
    }

    #[tokio::test]
    async fn update_provider_and_reload_requires_replacement_when_existing_api_key_is_unreadable() {
        let (_dir, repo, db) = make_repo();
        let manager = ProviderManager::new();
        let added = add_provider_and_reload(repo.as_ref(), &manager, sample_input("Primary", 20))
            .await
            .expect("add and reload");
        corrupt_api_key(&db, &added.id);

        let err = update_provider_and_reload(
            repo.as_ref(),
            &manager,
            &added.id,
            ProviderInput {
                api_key: String::new(),
                ..sample_input("Renamed", 5)
            },
        )
        .await
        .expect_err("blank key cannot replace unreadable key");

        assert!(err.contains("requires a replacement api key"));
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
    async fn delete_provider_and_reload_removes_unreadable_provider() {
        let (_dir, repo, db) = make_repo();
        let manager = ProviderManager::new();
        let added = add_provider_and_reload(repo.as_ref(), &manager, sample_input("Primary", 10))
            .await
            .expect("add and reload");
        corrupt_api_key(&db, &added.id);

        delete_provider_and_reload(repo.as_ref(), &manager, &added.id)
            .await
            .expect("delete unreadable provider");

        assert!(repo.get(&added.id).expect("provider lookup").is_none());
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
    async fn test_provider_connection_returns_structured_openai_stream_result() {
        let (_dir, repo, _db) = make_repo();
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(body_json(openai_probe_body("gpt-4o-mini")))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n"),
            )
            .expect(1)
            .mount(&server)
            .await;
        let added = repo
            .add(&ProviderInput {
                base_url: server.uri(),
                kind: ProviderKind::Openai,
                ..sample_input("Failing", 10)
            })
            .expect("add provider");

        let err = test_provider_connection_with_repo(repo.as_ref(), &added.id)
            .await
            .expect("probe result");

        assert!(err.success);
        assert_eq!(err.http_status, Some(200));
        assert_eq!(err.model_used, "gpt-4o-mini");
        assert!(matches!(
            err.status,
            StreamCheckStatus::Operational | StreamCheckStatus::Degraded
        ));
        assert!(err.response_time_ms.is_some());
    }

    #[test]
    fn provider_connection_test_timeout_is_30_seconds() {
        assert_eq!(
            PROVIDER_CONNECTION_TEST_TIMEOUT,
            std::time::Duration::from_secs(30),
        );
    }

    #[tokio::test]
    async fn test_provider_connection_returns_model_not_found_result() {
        let (_dir, repo, _db) = make_repo();
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .and(body_json(anthropic_probe_body("claude-3-5-sonnet-20241022")))
            .respond_with(
                ResponseTemplate::new(404).set_body_string(
                    r#"{"type":"error","error":{"type":"not_found_error","message":"model: claude-3-5-sonnet-20241022"}}"#,
                ),
            )
            .expect(1)
            .mount(&server)
            .await;
        let added = repo
            .add(&ProviderInput {
                base_url: server.uri(),
                kind: ProviderKind::Anthropic,
                ..sample_input("Failing", 10)
            })
            .expect("add provider");

        let result = test_provider_connection_with_repo(repo.as_ref(), &added.id)
            .await
            .expect("probe result");

        assert!(!result.success);
        assert_eq!(result.http_status, Some(404));
        assert_eq!(result.error_category.as_deref(), Some("modelNotFound"));
        assert_eq!(result.message, "Not found (404)");
        assert_eq!(result.model_used, "claude-3-5-sonnet-20241022");
        assert!(result.response_time_ms.is_some());
    }
}
