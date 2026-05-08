//! Backward-compatible model mapping type.
//!
//! Current product behavior does not apply model mappings. Provider
//! requests omit `model`, allowing the upstream provider or relay to use
//! its configured default model.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Deprecated model mapping container retained for old exports / command ABI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMapping {
    /// Key = canonical model name the client sends.
    /// Value = map of `vendor_kind` -> `actual_model_name`.
    entries: HashMap<String, HashMap<String, String>>,
}

impl Default for ModelMapping {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelMapping {
    /// Create an empty mapping table.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Look up the equivalent model for a target vendor.
    ///
    /// If `client_model` is `gpt-4o` and `target_vendor` is `anthropic`,
    /// returns `Some("claude-sonnet-4-20250514")`.
    #[must_use]
    pub fn map_model(&self, client_model: &str, target_vendor: &str) -> Option<String> {
        self.entries
            .get(client_model)
            .and_then(|m| m.get(target_vendor))
            .cloned()
    }

    /// Resolve a client-facing model for a concrete provider.
    ///
    /// Precedence is:
    /// 1. exact provider id (for per-provider overrides),
    /// 2. provider kind (`openai`, `anthropic`, `gemini`, ...),
    /// 3. global wildcard (`*`),
    /// 4. original client model.
    #[must_use]
    pub fn resolve_for_provider(
        &self,
        client_model: &str,
        provider_id: &str,
        provider_kind: &str,
    ) -> String {
        self.map_model(client_model, provider_id)
            .or_else(|| self.map_model(client_model, provider_kind))
            .or_else(|| self.map_model(client_model, "*"))
            .unwrap_or_else(|| client_model.to_owned())
    }

    /// Add or update a single mapping entry.
    pub fn set_mapping(&mut self, client_model: &str, vendor: &str, vendor_model: &str) {
        self.entries
            .entry(client_model.to_string())
            .or_default()
            .insert(vendor.to_string(), vendor_model.to_string());
    }

    /// Remove a mapping for a client model + vendor pair.
    pub fn remove_mapping(&mut self, client_model: &str, vendor: &str) {
        if let Some(m) = self.entries.get_mut(client_model) {
            m.remove(vendor);
            if m.is_empty() {
                self.entries.remove(client_model);
            }
        }
    }

    /// Return all mapping entries (for UI display).
    #[must_use]
    pub fn all_entries(&self) -> &HashMap<String, HashMap<String, String>> {
        &self.entries
    }

    /// Merge user overrides into this mapping (overrides win).
    pub fn merge(&mut self, overrides: &Self) {
        for (model, vendors) in &overrides.entries {
            let entry = self.entries.entry(model.clone()).or_default();
            for (vendor, name) in vendors {
                entry.insert(vendor.clone(), name.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_mapping_is_empty() {
        let mm = ModelMapping::new();
        assert!(mm.all_entries().is_empty());
    }

    #[test]
    fn unknown_model_returns_none() {
        let mm = ModelMapping::new();
        assert!(mm.map_model("unknown-model", "openai").is_none());
    }

    #[test]
    fn resolve_for_provider_prefers_exact_provider_id() {
        let mut mm = ModelMapping::new();
        mm.set_mapping("client-fast", "anthropic", "claude-kind-wide");
        mm.set_mapping("client-fast", "provider-a", "claude-provider-a");

        let resolved = mm.resolve_for_provider("client-fast", "provider-a", "anthropic");

        assert_eq!(resolved, "claude-provider-a");
    }

    #[test]
    fn resolve_for_provider_uses_provider_kind_fallback() {
        let mut mm = ModelMapping::new();
        mm.set_mapping("client-fast", "anthropic", "claude-kind-wide");

        let resolved = mm.resolve_for_provider("client-fast", "provider-b", "anthropic");

        assert_eq!(resolved, "claude-kind-wide");
    }

    #[test]
    fn resolve_for_provider_uses_global_wildcard_fallback() {
        let mut mm = ModelMapping::new();
        mm.set_mapping("client-fast", "*", "universal-model");

        let resolved = mm.resolve_for_provider("client-fast", "provider-c", "custom");

        assert_eq!(resolved, "universal-model");
    }

    #[test]
    fn resolve_for_provider_returns_original_model_without_match() {
        let mm = ModelMapping::new();

        let resolved = mm.resolve_for_provider("not-mapped", "provider-d", "custom");

        assert_eq!(resolved, "not-mapped");
    }

    #[test]
    fn set_and_get_custom_mapping() {
        let mut mm = ModelMapping::new();
        mm.set_mapping("my-model", "openai", "gpt-4o");
        assert_eq!(
            mm.map_model("my-model", "openai").as_deref(),
            Some("gpt-4o")
        );
    }

    #[test]
    fn remove_mapping() {
        let mut mm = ModelMapping::new();
        mm.set_mapping("test", "openai", "gpt-4o");
        mm.remove_mapping("test", "openai");
        assert!(mm.map_model("test", "openai").is_none());
    }

    #[test]
    fn merge_overrides() {
        let mut base = ModelMapping::new();
        let mut overrides = ModelMapping {
            entries: HashMap::new(),
        };
        overrides.set_mapping("gpt-4o", "anthropic", "claude-opus-4-20250514");

        base.merge(&overrides);
        // Override should win.
        assert_eq!(
            base.map_model("gpt-4o", "anthropic").as_deref(),
            Some("claude-opus-4-20250514")
        );
        assert!(base.map_model("gpt-4o", "gemini").is_none());
    }

    #[test]
    fn serialization_roundtrip() {
        let mm = ModelMapping::new();
        let json = serde_json::to_string(&mm).unwrap();
        let back: ModelMapping = serde_json::from_str(&json).unwrap();
        assert_eq!(
            mm.map_model("gpt-4o", "anthropic"),
            back.map_model("gpt-4o", "anthropic")
        );
    }

    #[test]
    fn all_entries_empty_by_default() {
        let mm = ModelMapping::new();
        assert!(mm.all_entries().is_empty());
    }
}
