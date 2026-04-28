//! Default model mapping table (T1.0.3.11).
//!
//! Maps model names across vendors so that a request for `gpt-4o` can
//! be routed to an Anthropic or Gemini provider with the equivalent
//! model automatically substituted.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Bidirectional model mapping between vendors.
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
    /// Create a mapping table with sensible defaults.
    #[must_use]
    pub fn new() -> Self {
        let mut entries = HashMap::new();

        // GPT-4o family.
        Self::add_group(
            &mut entries,
            &[
                ("openai", "gpt-4o"),
                ("anthropic", "claude-sonnet-4-20250514"),
                ("gemini", "gemini-2.5-flash"),
            ],
        );

        // GPT-4o-mini family.
        Self::add_group(
            &mut entries,
            &[
                ("openai", "gpt-4o-mini"),
                ("anthropic", "claude-haiku-4-5-20251001"),
                ("gemini", "gemini-2.0-flash"),
            ],
        );

        // GPT-4.1 family.
        Self::add_group(
            &mut entries,
            &[
                ("openai", "gpt-4.1"),
                ("anthropic", "claude-opus-4-20250514"),
                ("gemini", "gemini-2.5-pro"),
            ],
        );

        // o3-mini family.
        Self::add_group(
            &mut entries,
            &[
                ("openai", "o3-mini"),
                ("anthropic", "claude-sonnet-4-20250514"),
                ("gemini", "gemini-2.5-flash"),
            ],
        );

        Self { entries }
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

    fn add_group(entries: &mut HashMap<String, HashMap<String, String>>, group: &[(&str, &str)]) {
        // For every model in the group, create entries mapping to every
        // other vendor's model.  First-write wins so earlier groups
        // have higher priority.
        for &(_, model) in group {
            let map = entries.entry(model.to_string()).or_default();
            for &(vendor, vendor_model) in group {
                map.entry(vendor.to_string())
                    .or_insert_with(|| vendor_model.to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_mapping_gpt4o_to_anthropic() {
        let mm = ModelMapping::new();
        let result = mm.map_model("gpt-4o", "anthropic");
        assert_eq!(result.as_deref(), Some("claude-sonnet-4-20250514"));
    }

    #[test]
    fn default_mapping_claude_to_openai() {
        let mm = ModelMapping::new();
        let result = mm.map_model("claude-sonnet-4-20250514", "openai");
        assert_eq!(result.as_deref(), Some("gpt-4o"));
    }

    #[test]
    fn default_mapping_gemini_to_anthropic() {
        let mm = ModelMapping::new();
        let result = mm.map_model("gemini-2.5-flash", "anthropic");
        assert_eq!(result.as_deref(), Some("claude-sonnet-4-20250514"));
    }

    #[test]
    fn unknown_model_returns_none() {
        let mm = ModelMapping::new();
        assert!(mm.map_model("unknown-model", "openai").is_none());
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
        // Other mappings unaffected.
        assert_eq!(
            base.map_model("gpt-4o", "gemini").as_deref(),
            Some("gemini-2.5-flash")
        );
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
    fn all_entries_non_empty() {
        let mm = ModelMapping::new();
        assert!(!mm.all_entries().is_empty());
    }
}
