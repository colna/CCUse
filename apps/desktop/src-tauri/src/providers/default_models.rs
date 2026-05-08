//! Default outbound models used when the client request omits `model`.

use super::api::ProviderError;
use super::model::ProviderKind;

pub const OPENAI_DEFAULT_MODELS: &[&str] = &["gpt-5.5", "gpt-5.4", "gpt-5.4-mini"];
pub const ANTHROPIC_DEFAULT_MODELS: &[&str] =
    &["claude-opus-4.7", "claude-sonnet-4.6", "claude-haiku-4.5"];
pub const OPENAI_COMPATIBLE_DEFAULT_MODELS: &[&str] = &[
    "gpt-5.5",
    "gpt-5.4",
    "gpt-5.4-mini",
    "claude-opus-4.7",
    "claude-sonnet-4.6",
    "claude-haiku-4.5",
];
pub const GEMINI_DEFAULT_MODELS: &[&str] = &["gemini-3-flash-preview"];

#[must_use]
pub fn default_models_for_kind(kind: ProviderKind) -> &'static [&'static str] {
    match kind {
        ProviderKind::Openai => OPENAI_DEFAULT_MODELS,
        ProviderKind::Anthropic => ANTHROPIC_DEFAULT_MODELS,
        ProviderKind::Gemini => GEMINI_DEFAULT_MODELS,
        ProviderKind::Relay | ProviderKind::Custom => OPENAI_COMPATIBLE_DEFAULT_MODELS,
    }
}

#[must_use]
pub fn owned_defaults(defaults: &[&str]) -> Vec<String> {
    defaults.iter().map(|model| (*model).to_owned()).collect()
}

#[must_use]
pub fn model_candidates(_request_model: &str, defaults: &[String]) -> Vec<String> {
    defaults.to_vec()
}

#[must_use]
pub fn should_try_next_default_model(error: &ProviderError) -> bool {
    match error {
        ProviderError::BadRequest(body) | ProviderError::Upstream { body, .. } => {
            is_model_selection_error(body)
        }
        ProviderError::Network(_)
        | ProviderError::Unauthorized(_)
        | ProviderError::RateLimited(_)
        | ProviderError::Decode(_) => false,
    }
}

#[must_use]
fn is_model_selection_error(body: &str) -> bool {
    let lower = body.to_ascii_lowercase();
    lower.contains("model")
        && [
            "model is required",
            "model_not_found",
            "model not found",
            "invalid_model",
            "invalid model",
            "unknown_model",
            "unknown model",
            "does not exist",
            "not a valid model",
            "not supported",
            "not found",
            "no access",
            "permission",
        ]
        .iter()
        .any(|needle| lower.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_request_model_uses_default_candidates() {
        let defaults = owned_defaults(OPENAI_DEFAULT_MODELS);

        assert_eq!(
            model_candidates("", &defaults),
            vec!["gpt-5.5", "gpt-5.4", "gpt-5.4-mini"],
        );
    }

    #[test]
    fn explicit_request_model_is_ignored_for_provider_defaults() {
        let defaults = owned_defaults(OPENAI_DEFAULT_MODELS);

        assert_eq!(
            model_candidates(" gpt-custom ", &defaults),
            vec!["gpt-5.5", "gpt-5.4", "gpt-5.4-mini"],
        );
    }

    #[test]
    fn model_not_found_errors_are_default_retryable() {
        let err = ProviderError::BadRequest(
            r#"{"error":{"message":"The model `gpt-5.5` does not exist"}}"#.to_owned(),
        );

        assert!(should_try_next_default_model(&err));
    }

    #[test]
    fn generic_bad_requests_do_not_try_next_default_model() {
        let err = ProviderError::BadRequest("messages is required".to_owned());

        assert!(!should_try_next_default_model(&err));
    }
}
