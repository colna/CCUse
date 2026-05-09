//! Default outbound models used when the client request omits `model`.

use super::api::ProviderError;
use super::model::ProviderKind;

pub const OPENAI_DEFAULT_MODELS: &[&str] = &["gpt-5.5", "gpt-5.4", "gpt-5.4"];
pub const ANTHROPIC_DEFAULT_MODELS: &[&str] = &[
    "claude-opus-4-7",
    "claude-sonnet-4-6",
    "claude-haiku-4-5-20251001",
];
pub const OPENAI_COMPATIBLE_DEFAULT_MODELS: &[&str] = &[
    "gpt-5.5",
    "gpt-5.4",
    "gpt-5.4",
    "claude-opus-4-7",
    "claude-sonnet-4-6",
    "claude-haiku-4-5-20251001",
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
    defaults
        .iter()
        .map(|model| model.trim().to_owned())
        .collect()
}

#[must_use]
pub fn model_candidates(request_model: &str, defaults: &[String]) -> Vec<String> {
    let request_model = request_model.trim();
    let mut candidates = Vec::new();

    if !request_model.is_empty() {
        candidates.push(request_model.to_owned());
    }

    candidates.extend(
        defaults
            .iter()
            .map(|model| model.trim())
            .filter(|model| !model.is_empty())
            .filter(|model| *model != request_model)
            .map(str::to_owned),
    );
    candidates
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
            vec!["gpt-5.5", "gpt-5.4", "gpt-5.4"],
        );
    }

    #[test]
    fn explicit_request_model_is_tried_before_provider_defaults() {
        let defaults = owned_defaults(OPENAI_DEFAULT_MODELS);

        assert_eq!(
            model_candidates(" gpt-custom ", &defaults),
            vec!["gpt-custom", "gpt-5.5", "gpt-5.4", "gpt-5.4",],
        );
    }

    #[test]
    fn explicit_request_model_is_not_duplicated_when_already_a_default() {
        let defaults = owned_defaults(OPENAI_DEFAULT_MODELS);

        assert_eq!(
            model_candidates(" gpt-5.5 ", &defaults),
            vec!["gpt-5.5", "gpt-5.4", "gpt-5.4"],
        );
    }

    #[test]
    fn default_candidates_trim_accidental_whitespace() {
        let defaults = vec![" gpt-5.5 ".to_owned(), " ".to_owned(), "gpt-5.4".to_owned()];

        assert_eq!(model_candidates("", &defaults), vec!["gpt-5.5", "gpt-5.4"],);
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
