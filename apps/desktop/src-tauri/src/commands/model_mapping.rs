//! Backward-compatible no-op commands for the removed model mapping UI.

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
    _mapping: tauri::State<'_, ModelMappingHandle>,
) -> Result<Vec<MappingEntry>, String> {
    Ok(Vec::new())
}

#[tauri::command]
pub async fn set_model_mapping(
    _mapping: tauri::State<'_, ModelMappingHandle>,
    _client_model: String,
    _vendor: String,
    _vendor_model: String,
) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn remove_model_mapping(
    _mapping: tauri::State<'_, ModelMappingHandle>,
    _client_model: String,
    _vendor: String,
) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn get_model_mappings_returns_empty_after_feature_removal() {
        let handle: ModelMappingHandle = Arc::new(RwLock::new(ModelMapping::new()));
        assert!(handle.read().await.all_entries().is_empty());
    }

    #[tokio::test]
    async fn model_mapping_defaults_to_empty() {
        let handle: ModelMappingHandle = Arc::new(RwLock::new(ModelMapping::new()));
        assert!(handle.read().await.all_entries().is_empty());
    }
}
