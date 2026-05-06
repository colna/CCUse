//! Provider domain types — what's persisted in `providers` and
//! returned to the UI / `SwitchEngine`.

use serde::{Deserialize, Serialize};

/// Upstream protocol family. The string form is what's stored in
/// the `kind` column and shown in the "Add provider" dropdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    /// `OpenAI`-compatible (`/v1/chat/completions`).
    Openai,
    /// Anthropic (`/v1/messages`).
    Anthropic,
    /// Claude-compatible relays that expose Anthropic Messages.
    Claude,
    /// Google Gemini (`/v1beta/models/.../generateContent`).
    Gemini,
    /// Relay / proxy endpoint (e.g. `OpenRouter`, One API).
    Relay,
    /// Generic OpenAI-compatible endpoint (e.g. self-hosted).
    Custom,
}

impl ProviderKind {
    /// Stable wire string. Used in the `kind` column and in JSON.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Openai => "openai",
            Self::Anthropic => "anthropic",
            Self::Claude => "claude",
            Self::Gemini => "gemini",
            Self::Relay => "relay",
            Self::Custom => "custom",
        }
    }

    /// The outbound wire protocol used by the runtime provider.
    #[must_use]
    pub const fn protocol_vendor(self) -> &'static str {
        match self {
            Self::Openai | Self::Relay => "openai",
            Self::Anthropic | Self::Claude | Self::Custom => "anthropic",
            Self::Gemini => "gemini",
        }
    }

    /// Whether runtime dispatch should use Anthropic Messages (`/v1/messages`).
    #[must_use]
    pub const fn uses_anthropic_messages(self) -> bool {
        matches!(self, Self::Anthropic | Self::Claude | Self::Custom)
    }

    /// Reverse of [`Self::as_str`]. `None` if `value` doesn't match
    /// any known kind — caller decides whether to error or fallback.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "openai" => Some(Self::Openai),
            "anthropic" => Some(Self::Anthropic),
            "claude" => Some(Self::Claude),
            "gemini" => Some(Self::Gemini),
            "relay" => Some(Self::Relay),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
}

/// Caller-supplied data when creating or updating a provider. The
/// API key is plaintext here; the repository encrypts it on write.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInput {
    /// Display name shown in the UI (e.g. `Work OpenAI`).
    pub name: String,
    pub kind: ProviderKind,
    /// Base URL minus trailing slash, e.g. `https://api.openai.com`.
    pub base_url: String,
    /// Plaintext API key. Lives in this struct only for the duration
    /// of the call — repository encrypts immediately.
    pub api_key: String,
    /// Lower numbers = higher priority. Default 100 mirrors the SQL.
    pub priority: i32,
    pub enabled: bool,
    /// Monthly token quota (optional cap).
    pub monthly_quota: Option<i64>,
    /// Requests per minute limit.
    pub rate_limit_rpm: Option<i32>,
    /// Cost per 1 000 tokens (USD).
    pub cost_per_1k_tokens: Option<f64>,
}

/// Persisted provider. The plaintext API key is intentionally absent
/// — UI surfaces a "*** rotate" affordance instead of showing it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub kind: ProviderKind,
    pub base_url: String,
    pub priority: i32,
    pub enabled: bool,
    pub monthly_quota: Option<i64>,
    pub rate_limit_rpm: Option<i32>,
    pub cost_per_1k_tokens: Option<f64>,
    pub created_at: String,
    pub updated_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_kind_round_trips_through_strings() {
        for kind in [
            ProviderKind::Openai,
            ProviderKind::Anthropic,
            ProviderKind::Claude,
            ProviderKind::Gemini,
            ProviderKind::Relay,
            ProviderKind::Custom,
        ] {
            let s = kind.as_str();
            assert_eq!(ProviderKind::parse(s), Some(kind));
        }
    }

    #[test]
    fn unknown_kind_string_returns_none() {
        assert!(ProviderKind::parse("openrouter").is_none());
        assert!(ProviderKind::parse("").is_none());
    }
}
