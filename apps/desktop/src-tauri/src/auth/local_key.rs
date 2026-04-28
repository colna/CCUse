//! Generate the `sk-local-{32-char}` token the local proxy hands out.
//!
//! The desktop app issues one of these on first launch and lets the
//! user copy it into Cursor / Claude Desktop / Continue. Format mirrors
//! `OpenAI`'s `sk-…` shape so generic clients accept it without
//! special-casing. Validation lives in T1.0.1.13.

use rand::distr::Alphanumeric;
use rand::Rng;

/// Stable prefix that identifies a CCUse-issued local key. Anything
/// without it is treated as "not ours" by the auth middleware.
pub const LOCAL_API_KEY_PREFIX: &str = "sk-local-";

/// Length of the random suffix. 32 alphanumeric chars ≈ 190 bits of
/// entropy — enough that brute force isn't a realistic threat for a
/// loopback-only service.
pub const RANDOM_PART_LEN: usize = 32;

/// A freshly generated local API key. Wrap so we can change the
/// internal representation later (zeroizing buffer, secrecy crate)
/// without touching call sites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalApiKey(String);

impl LocalApiKey {
    /// Borrow the key as `&str` for HTTP responses / clipboard copy.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Move the key out as `String`. Used by the keyring/storage layer
    /// when it needs an owned copy.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl std::fmt::Display for LocalApiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Generate a fresh `sk-local-{32 alphanumeric}` token.
///
/// Uses [`rand::rng`] (the OS-seeded thread-local RNG in `rand` 0.9)
/// so callers don't need to plumb a generator. Each call yields a
/// fresh random suffix; the prefix is constant.
#[must_use]
pub fn generate_local_api_key() -> LocalApiKey {
    let mut buf = String::with_capacity(LOCAL_API_KEY_PREFIX.len() + RANDOM_PART_LEN);
    buf.push_str(LOCAL_API_KEY_PREFIX);
    for ch in rand::rng()
        .sample_iter(Alphanumeric)
        .take(RANDOM_PART_LEN)
        .map(char::from)
    {
        buf.push(ch);
    }
    LocalApiKey(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn generated_key_starts_with_sk_local_prefix() {
        let key = generate_local_api_key();
        assert!(
            key.as_str().starts_with(LOCAL_API_KEY_PREFIX),
            "key must start with `{LOCAL_API_KEY_PREFIX}`, got `{}`",
            key.as_str(),
        );
    }

    #[test]
    fn generated_key_has_exact_total_length() {
        let key = generate_local_api_key();
        assert_eq!(
            key.as_str().len(),
            LOCAL_API_KEY_PREFIX.len() + RANDOM_PART_LEN,
        );
    }

    #[test]
    fn random_suffix_is_alphanumeric_only() {
        let key = generate_local_api_key();
        let suffix = &key.as_str()[LOCAL_API_KEY_PREFIX.len()..];
        assert!(
            suffix.chars().all(|c| c.is_ascii_alphanumeric()),
            "suffix `{suffix}` must be ascii alphanumeric",
        );
    }

    #[test]
    fn successive_keys_are_unique() {
        // 1024 draws from a 62-char alphabet × 32 chars: collision is
        // astronomically unlikely. If this fires the RNG is broken.
        const N: usize = 1024;
        let unique: HashSet<String> = (0..N)
            .map(|_| generate_local_api_key().into_string())
            .collect();
        assert_eq!(unique.len(), N, "all generated keys should be unique");
    }

    #[test]
    fn display_renders_full_token() {
        let key = generate_local_api_key();
        assert_eq!(format!("{key}"), key.as_str());
    }
}
