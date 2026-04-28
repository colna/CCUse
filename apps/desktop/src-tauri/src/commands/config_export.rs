//! T1.0.4.18–20 — Config export / import / template preset commands.
//!
//! * `export_config_json` — serialises current config + encrypts with password
//! * `import_config_json` — decrypts + applies imported config
//! * `get_template_presets` — returns 3 built-in quick-start presets

use tauri::State;

use crate::commands::providers::ProviderRepoHandle;
use crate::commands::switch::SwitchEngineHandle;
use crate::config_export::{
    decrypt_export, encrypt_export, template_presets, ExportData, ExportProvider, TemplatePreset,
};
use crate::providers::model::ProviderInput;

use super::model_mapping::ModelMappingHandle;

/// Serialise current config and encrypt it with the user-supplied password.
///
/// Returns the raw encrypted blob (frontend saves it as a `.ccuse` file).
#[tauri::command]
pub async fn export_config_json(
    password: String,
    repo: State<'_, ProviderRepoHandle>,
    engine: State<'_, SwitchEngineHandle>,
    mapping: State<'_, ModelMappingHandle>,
) -> Result<Vec<u8>, String> {
    let providers = repo.list().map_err(|e| e.to_string())?;
    let export_providers: Vec<ExportProvider> =
        providers.iter().map(ExportProvider::from).collect();

    let config = engine.config().await;
    let model_mapping = mapping.read().await.clone();

    let data = ExportData {
        providers: export_providers,
        strategy: config.strategy,
        smart_weights: config.smart_weights,
        model_mapping,
    };

    let json = serde_json::to_vec(&data).map_err(|e| e.to_string())?;
    encrypt_export(&json, &password).map_err(|e| e.to_string())
}

/// Decrypt an imported blob and apply the config.
///
/// Providers in the import are added (not merged) — duplicates by name
/// are skipped. Strategy and model mappings are overwritten.
#[tauri::command]
pub async fn import_config_json(
    data: Vec<u8>,
    password: String,
    repo: State<'_, ProviderRepoHandle>,
    engine: State<'_, SwitchEngineHandle>,
    mapping: State<'_, ModelMappingHandle>,
) -> Result<(), String> {
    let plaintext = decrypt_export(&data, &password).map_err(|e| e.to_string())?;
    let export: ExportData = serde_json::from_slice(&plaintext).map_err(|e| e.to_string())?;

    let existing = repo.list().map_err(|e| e.to_string())?;
    let existing_names: std::collections::HashSet<String> =
        existing.iter().map(|p| p.name.clone()).collect();

    for ep in &export.providers {
        if existing_names.contains(&ep.name) {
            continue;
        }
        let input = ProviderInput {
            name: ep.name.clone(),
            kind: ep.kind,
            base_url: ep.base_url.clone(),
            api_key: String::new(),
            priority: ep.priority,
            enabled: ep.enabled,
            monthly_quota: ep.monthly_quota,
            rate_limit_rpm: ep.rate_limit_rpm,
            cost_per_1k_tokens: ep.cost_per_1k_tokens,
        };
        repo.add(&input).map_err(|e| e.to_string())?;
    }

    engine.set_strategy(export.strategy).await;
    engine.set_smart_weights(export.smart_weights).await;

    {
        let mut mm = mapping.write().await;
        mm.merge(&export.model_mapping);
    }

    Ok(())
}

/// Return the 3 built-in template presets.
#[tauri::command]
pub async fn get_template_presets() -> Result<Vec<TemplatePreset>, String> {
    Ok(template_presets())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_export::ExportData;
    use crate::converter::ModelMapping;
    use crate::switch::strategy::SmartWeights;
    use crate::switch::SwitchStrategy;

    #[test]
    fn export_data_round_trip_through_encryption() {
        let data = ExportData {
            providers: vec![ExportProvider {
                name: "Test".into(),
                kind: crate::providers::model::ProviderKind::Openai,
                base_url: "https://api.openai.com".into(),
                priority: 10,
                enabled: true,
                monthly_quota: None,
                rate_limit_rpm: None,
                cost_per_1k_tokens: None,
            }],
            strategy: SwitchStrategy::Priority,
            smart_weights: SmartWeights::default(),
            model_mapping: ModelMapping::new(),
        };
        let json = serde_json::to_vec(&data).expect("serialize");
        let blob = encrypt_export(&json, "test-pw").expect("encrypt");
        let recovered = decrypt_export(&blob, "test-pw").expect("decrypt");
        let back: ExportData = serde_json::from_slice(&recovered).expect("deserialize");
        assert_eq!(back.providers.len(), 1);
        assert_eq!(back.providers[0].name, "Test");
    }

    #[test]
    fn presets_have_valid_data() {
        let presets = template_presets();
        for preset in &presets {
            assert!(!preset.id.is_empty());
            assert!(!preset.name.is_empty());
            assert!(!preset.providers.is_empty());
            for p in &preset.providers {
                assert!(!p.base_url.is_empty());
                assert!(p.base_url.starts_with("https://"));
            }
        }
    }
}
