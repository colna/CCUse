//! Tauri commands for model mapping (T1.0.3.12).

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::converter::ModelMapping;

/// Shared model mapping state.
pub type ModelMappingHandle = Arc<RwLock<ModelMapping>>;

/// Entry shape returned to the frontend.
#[derive(Debug, serde::Serialize)]
pub struct MappingEntry {
    pub client_model: String,
    pub openai: Option<String>,
    pub anthropic: Option<String>,
    pub gemini: Option<String>,
}

#[tauri::command]
pub async fn get_model_mappings(
    mapping: tauri::State<'_, ModelMappingHandle>,
) -> Result<Vec<MappingEntry>, String> {
    let mm = mapping.read().await;
    let entries = mm.all_entries();
    let mut result: Vec<MappingEntry> = entries
        .iter()
        .map(|(model, vendors)| MappingEntry {
            client_model: model.clone(),
            openai: vendors.get("openai").cloned(),
            anthropic: vendors.get("anthropic").cloned(),
            gemini: vendors.get("gemini").cloned(),
        })
        .collect();
    result.sort_by(|a, b| a.client_model.cmp(&b.client_model));
    Ok(result)
}

#[tauri::command]
pub async fn set_model_mapping(
    mapping: tauri::State<'_, ModelMappingHandle>,
    client_model: String,
    vendor: String,
    vendor_model: String,
) -> Result<(), String> {
    let mut mm = mapping.write().await;
    mm.set_mapping(&client_model, &vendor, &vendor_model);
    Ok(())
}

#[tauri::command]
pub async fn remove_model_mapping(
    mapping: tauri::State<'_, ModelMappingHandle>,
    client_model: String,
    vendor: String,
) -> Result<(), String> {
    let mut mm = mapping.write().await;
    mm.remove_mapping(&client_model, &vendor);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn get_default_mappings() {
        let handle: ModelMappingHandle = Arc::new(RwLock::new(ModelMapping::new()));
        let entries = {
            let mm = handle.read().await;
            let entries = mm.all_entries();
            let mut result: Vec<MappingEntry> = entries
                .iter()
                .map(|(model, vendors)| MappingEntry {
                    client_model: model.clone(),
                    openai: vendors.get("openai").cloned(),
                    anthropic: vendors.get("anthropic").cloned(),
                    gemini: vendors.get("gemini").cloned(),
                })
                .collect();
            result.sort_by(|a, b| a.client_model.cmp(&b.client_model));
            result
        };
        assert!(!entries.is_empty());
        // gpt-4o should exist.
        assert!(entries.iter().any(|e| e.client_model == "gpt-4o"));
    }

    #[tokio::test]
    async fn set_and_get() {
        let handle: ModelMappingHandle = Arc::new(RwLock::new(ModelMapping::new()));
        {
            let mut mm = handle.write().await;
            mm.set_mapping("test-model", "openai", "gpt-4o");
        }
        let mm = handle.read().await;
        assert_eq!(
            mm.map_model("test-model", "openai").as_deref(),
            Some("gpt-4o")
        );
    }
}
