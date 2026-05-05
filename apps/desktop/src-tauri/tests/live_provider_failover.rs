//! Live provider diagnostics for a developer machine.
//!
//! This test is intentionally gated behind `CCUSE_LIVE_PROVIDER_TEST=1` because it
//! reads the local app database, decrypts provider keys in memory, and sends real
//! network requests. It never prints API keys.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ccuse_desktop_lib::crypto::{load_or_create_master_key, FileKeyringBackend};
use ccuse_desktop_lib::db;
use ccuse_desktop_lib::providers::api::{ApiRequest, ChatMessage, Provider as _, ProviderError};
use ccuse_desktop_lib::providers::model::ProviderKind;
use ccuse_desktop_lib::providers::openai::OpenAIProvider;
use ccuse_desktop_lib::providers::repository::ProviderRepository;
use ccuse_desktop_lib::providers::wrapper::ProviderWrapper;
use ccuse_desktop_lib::providers::ProviderManager;
use ccuse_desktop_lib::switch::{SwitchConfig, SwitchEngine, SwitchStrategy};

fn live_enabled() -> bool {
    std::env::var("CCUSE_LIVE_PROVIDER_TEST").as_deref() == Ok("1")
}

fn app_data_dir() -> PathBuf {
    std::env::var_os("CCUSE_APP_DATA_DIR").map_or_else(
        || {
            let home = std::env::var_os("HOME").expect("HOME must be set");
            PathBuf::from(home).join("Library/Application Support/io.ccuse.desktop")
        },
        PathBuf::from,
    )
}

fn load_live_repo(app_dir: &Path) -> ProviderRepository {
    let keyring_path = app_dir.join("keyring_fallback.json");
    assert!(
        keyring_path.exists(),
        "missing {}, refusing to create a new diagnostic key store",
        keyring_path.display()
    );
    let db_path = app_dir.join("ccuse.db");
    let database = db::open_database(&db_path).expect("open live app db");
    let backend = FileKeyringBackend::new(keyring_path);
    let master_key = Arc::new(load_or_create_master_key(&backend).expect("load app master key"));
    ProviderRepository::new(database, master_key)
}

fn tiny_chat(model: &str, stream: bool) -> ApiRequest {
    ApiRequest {
        model: model.to_owned(),
        messages: vec![ChatMessage {
            role: "user".to_owned(),
            content: "Reply with exactly: ok".to_owned(),
            tool_call_id: None,
            tool_calls: vec![],
        }],
        temperature: Some(0.0),
        max_tokens: Some(8),
        stream,
        tools: vec![],
    }
}

fn summarize_error(error: &ProviderError) -> String {
    match error {
        ProviderError::Network(_) => "network".to_owned(),
        ProviderError::Upstream { status, .. } => format!("upstream_{status}"),
        ProviderError::Unauthorized(_) => "unauthorized".to_owned(),
        ProviderError::RateLimited(_) => "rate_limited".to_owned(),
        ProviderError::Decode(_) => "decode".to_owned(),
        ProviderError::BadRequest(_) => "bad_request".to_owned(),
    }
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn live_provider_failover_diagnostic() {
    if !live_enabled() {
        eprintln!("skipping live provider diagnostic; set CCUSE_LIVE_PROVIDER_TEST=1");
        return;
    }

    let repo = load_live_repo(&app_data_dir());
    let providers = repo.list().expect("list live providers");
    println!("configured providers: {}", providers.len());
    for provider in &providers {
        let key = repo
            .get_decrypted_api_key(&provider.id)
            .unwrap_or_else(|err| panic!("{} key decrypt failed: {err}", provider.name));
        println!(
            "provider {} priority={} enabled={} url={} key_decrypt=ok",
            provider.name, provider.priority, provider.enabled, provider.base_url
        );
        drop(key);
    }

    let manager = Arc::new(ProviderManager::new());
    let loaded = manager
        .load_from_repository(&repo)
        .await
        .expect("runtime provider load");
    println!("runtime loaded providers: {loaded}");

    let runtime_providers = manager.enabled_by_priority().await;
    assert!(
        runtime_providers.len() >= 2,
        "need at least two enabled runtime providers for failover diagnostics"
    );

    for provider in &runtime_providers {
        match provider.list_models().await {
            Ok(models) => println!(
                "models {} ok count={} sample={}",
                provider.name(),
                models.len(),
                models.first().map_or("<empty>", |model| model.id.as_str())
            ),
            Err(err) => println!("models {} error={}", provider.name(), summarize_error(&err)),
        }
    }

    let model = std::env::var("CCUSE_LIVE_MODEL").unwrap_or_else(|_| "claude-opus-4-6".to_owned());
    let config = SwitchConfig {
        strategy: SwitchStrategy::Priority,
        max_retries: runtime_providers.len(),
        ..SwitchConfig::default()
    };
    let engine = SwitchEngine::with_config(Arc::clone(&manager), config);
    match engine.dispatch(tiny_chat(&model, false)).await {
        Ok(result) => println!(
            "normal dispatch ok provider={} attempts={}",
            result.provider_name, result.attempts
        ),
        Err(failure) => println!(
            "normal dispatch error provider={} kind={}",
            failure.provider_name.as_deref().unwrap_or("<none>"),
            summarize_error(&failure.error)
        ),
    }

    let first = &providers[0];
    let first_bad = Arc::new(ProviderWrapper::new(
        "diagnostic-bad-primary",
        "diagnostic bad primary",
        ProviderKind::Custom,
        first.priority - 1,
        None,
        true,
        Box::new(
            OpenAIProvider::new(
                "diagnostic-bad-primary",
                "diagnostic bad primary",
                &first.base_url,
                "ccuse-diagnostic-invalid-key",
            )
            .expect("build bad primary"),
        ),
    ));

    let failover_manager = Arc::new(ProviderManager::new());
    failover_manager
        .add(first_bad)
        .await
        .expect("add bad primary");
    for provider in runtime_providers {
        failover_manager
            .add(provider)
            .await
            .expect("add live provider");
    }

    let config = SwitchConfig {
        strategy: SwitchStrategy::Priority,
        max_retries: providers.len() + 1,
        ..SwitchConfig::default()
    };
    let failover_engine = SwitchEngine::with_config(failover_manager, config);
    match failover_engine.dispatch(tiny_chat(&model, false)).await {
        Ok(result) => println!(
            "forced failover ok provider={} attempts={} switched_from={:?} reason={:?}",
            result.provider_name,
            result.attempts,
            result.switched_from_provider_id,
            result.switch_reason
        ),
        Err(failure) => panic!(
            "forced failover failed provider={} kind={}",
            failure.provider_name.as_deref().unwrap_or("<none>"),
            summarize_error(&failure.error)
        ),
    }
}
