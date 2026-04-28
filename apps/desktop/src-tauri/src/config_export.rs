//! Config export / import with password-based encryption (T1.0.4.18–20).
//!
//! The export wire format is:
//!
//! ```text
//!   4 bytes   magic   "CCEX"
//!   1 byte    version (0x01)
//!  32 bytes   salt    (random, for scrypt KDF)
//!  12 bytes   nonce   (random, for AES-256-GCM)
//!   N bytes   ciphertext (JSON payload + 16-byte GCM tag)
//! ```
//!
//! The KDF is scrypt (`log_n=15`, `r=8`, `p=1`) producing a 32-byte key
//! from the user-supplied password + random salt. The payload is
//! AES-256-GCM encrypted.

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use rand::rngs::OsRng;
use rand::TryRngCore;
use serde::{Deserialize, Serialize};

use crate::converter::ModelMapping;
use crate::providers::model::{Provider, ProviderKind};
use crate::switch::strategy::{SmartWeights, SwitchStrategy};

/// Magic bytes at the start of every export file.
const MAGIC: &[u8; 4] = b"CCEX";
/// Wire-format version.
const VERSION: u8 = 0x01;
/// scrypt salt length.
const SALT_LEN: usize = 32;
/// AES-GCM nonce length (96 bits).
const NONCE_LEN: usize = 12;
/// Header size = magic(4) + version(1) + salt(32) + nonce(12) = 49.
const HEADER_LEN: usize = 4 + 1 + SALT_LEN + NONCE_LEN;

/// Exported provider shape (API keys stripped).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportProvider {
    pub name: String,
    pub kind: ProviderKind,
    pub base_url: String,
    pub priority: i32,
    pub enabled: bool,
    pub monthly_quota: Option<i64>,
    pub rate_limit_rpm: Option<i32>,
    pub cost_per_1k_tokens: Option<f64>,
}

impl From<&Provider> for ExportProvider {
    fn from(p: &Provider) -> Self {
        Self {
            name: p.name.clone(),
            kind: p.kind,
            base_url: p.base_url.clone(),
            priority: p.priority,
            enabled: p.enabled,
            monthly_quota: p.monthly_quota,
            rate_limit_rpm: p.rate_limit_rpm,
            cost_per_1k_tokens: p.cost_per_1k_tokens,
        }
    }
}

/// The JSON payload inside the encrypted envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportData {
    pub providers: Vec<ExportProvider>,
    pub strategy: SwitchStrategy,
    pub smart_weights: SmartWeights,
    pub model_mapping: ModelMapping,
}

/// A quick-start template preset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplatePreset {
    pub id: String,
    pub name: String,
    pub description: String,
    pub providers: Vec<ExportProvider>,
}

/// Errors from config export / import operations.
#[derive(thiserror::Error, Debug)]
pub enum ConfigExportError {
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("OS random failure: {0}")]
    Random(String),
    #[error("scrypt KDF failure: {0}")]
    Kdf(String),
    #[error("encryption failed")]
    Encrypt,
    #[error("decryption failed (wrong password or corrupted file)")]
    Decrypt,
    #[error("invalid export file: {0}")]
    InvalidFormat(String),
}

/// Derive a 32-byte AES key from `password` + `salt` using scrypt.
fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; 32], ConfigExportError> {
    let params =
        scrypt::Params::new(15, 8, 1, 32).map_err(|e| ConfigExportError::Kdf(e.to_string()))?;
    let mut key = [0u8; 32];
    scrypt::scrypt(password.as_bytes(), salt, &params, &mut key)
        .map_err(|e| ConfigExportError::Kdf(e.to_string()))?;
    Ok(key)
}

/// Encrypt `data` with `password` using scrypt KDF + AES-256-GCM.
///
/// Returns the full wire-format blob (magic + version + salt + nonce + ciphertext).
pub fn encrypt_export(data: &[u8], password: &str) -> Result<Vec<u8>, ConfigExportError> {
    let mut salt = [0u8; SALT_LEN];
    OsRng
        .try_fill_bytes(&mut salt)
        .map_err(|e| ConfigExportError::Random(e.to_string()))?;
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng
        .try_fill_bytes(&mut nonce_bytes)
        .map_err(|e| ConfigExportError::Random(e.to_string()))?;

    let key = derive_key(password, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key).expect("key is 32 bytes");
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, data)
        .map_err(|_| ConfigExportError::Encrypt)?;

    let mut out = Vec::with_capacity(HEADER_LEN + ciphertext.len());
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    out.extend_from_slice(&salt);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypt a wire-format blob produced by [`encrypt_export`].
///
/// Returns the original plaintext bytes.
pub fn decrypt_export(blob: &[u8], password: &str) -> Result<Vec<u8>, ConfigExportError> {
    if blob.len() < HEADER_LEN + 16 {
        return Err(ConfigExportError::InvalidFormat(format!(
            "file too short ({} bytes)",
            blob.len()
        )));
    }
    if &blob[..4] != MAGIC {
        return Err(ConfigExportError::InvalidFormat(
            "missing CCEX magic bytes".into(),
        ));
    }
    if blob[4] != VERSION {
        return Err(ConfigExportError::InvalidFormat(format!(
            "unsupported version 0x{:02x}",
            blob[4]
        )));
    }
    let salt = &blob[5..5 + SALT_LEN];
    let nonce_bytes = &blob[5 + SALT_LEN..HEADER_LEN];
    let ciphertext = &blob[HEADER_LEN..];

    let key = derive_key(password, salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key).expect("key is 32 bytes");
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| ConfigExportError::Decrypt)
}

/// Build the three built-in template presets.
pub fn template_presets() -> Vec<TemplatePreset> {
    vec![
        TemplatePreset {
            id: "claude".into(),
            name: "Claude (Anthropic)".into(),
            description: "Anthropic Claude with Messages API".into(),
            providers: vec![ExportProvider {
                name: "Anthropic".into(),
                kind: ProviderKind::Anthropic,
                base_url: "https://api.anthropic.com".into(),
                priority: 10,
                enabled: true,
                monthly_quota: None,
                rate_limit_rpm: None,
                cost_per_1k_tokens: Some(0.015),
            }],
        },
        TemplatePreset {
            id: "openai".into(),
            name: "OpenAI".into(),
            description: "OpenAI GPT models via Chat Completions API".into(),
            providers: vec![ExportProvider {
                name: "OpenAI".into(),
                kind: ProviderKind::Openai,
                base_url: "https://api.openai.com".into(),
                priority: 10,
                enabled: true,
                monthly_quota: None,
                rate_limit_rpm: None,
                cost_per_1k_tokens: Some(0.01),
            }],
        },
        TemplatePreset {
            id: "gemini".into(),
            name: "Google Gemini".into(),
            description: "Google Gemini via Vertex / generativelanguage API".into(),
            providers: vec![ExportProvider {
                name: "Gemini".into(),
                kind: ProviderKind::Gemini,
                base_url: "https://generativelanguage.googleapis.com".into(),
                priority: 10,
                enabled: true,
                monthly_quota: None,
                rate_limit_rpm: None,
                cost_per_1k_tokens: Some(0.00025),
            }],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_round_trip() {
        let plaintext = b"hello config data";
        let password = "s3cret!";
        let blob = encrypt_export(plaintext, password).expect("encrypt");
        let recovered = decrypt_export(&blob, password).expect("decrypt");
        assert_eq!(recovered, plaintext);
    }

    #[test]
    fn wrong_password_fails() {
        let blob = encrypt_export(b"data", "correct").expect("encrypt");
        let err = decrypt_export(&blob, "wrong").expect_err("must fail");
        assert!(matches!(err, ConfigExportError::Decrypt));
    }

    #[test]
    fn magic_bytes_present() {
        let blob = encrypt_export(b"x", "pw").expect("encrypt");
        assert_eq!(&blob[..4], b"CCEX");
        assert_eq!(blob[4], VERSION);
    }

    #[test]
    fn truncated_blob_rejected() {
        let blob = encrypt_export(b"x", "pw").expect("encrypt");
        let cut = &blob[..10];
        let err = decrypt_export(cut, "pw").expect_err("must fail");
        assert!(matches!(err, ConfigExportError::InvalidFormat(_)));
    }

    #[test]
    fn bad_magic_rejected() {
        let mut blob = encrypt_export(b"x", "pw").expect("encrypt");
        blob[0] = b'Z';
        let err = decrypt_export(&blob, "pw").expect_err("must fail");
        assert!(matches!(err, ConfigExportError::InvalidFormat(_)));
    }

    #[test]
    fn bad_version_rejected() {
        let mut blob = encrypt_export(b"x", "pw").expect("encrypt");
        blob[4] = 0xFF;
        let err = decrypt_export(&blob, "pw").expect_err("must fail");
        assert!(matches!(err, ConfigExportError::InvalidFormat(_)));
    }

    #[test]
    fn export_data_serialization_round_trip() {
        let data = ExportData {
            providers: vec![ExportProvider {
                name: "Test".into(),
                kind: ProviderKind::Openai,
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
        let back: ExportData = serde_json::from_slice(&json).expect("deserialize");
        assert_eq!(back.providers.len(), 1);
        assert_eq!(back.strategy, SwitchStrategy::Priority);
    }

    #[test]
    fn template_presets_returns_three() {
        let presets = template_presets();
        assert_eq!(presets.len(), 3);
        assert_eq!(presets[0].id, "claude");
        assert_eq!(presets[1].id, "openai");
        assert_eq!(presets[2].id, "gemini");
    }

    #[test]
    fn export_provider_from_provider() {
        let p = Provider {
            id: "id-1".into(),
            name: "Test".into(),
            kind: ProviderKind::Openai,
            base_url: "https://api.openai.com".into(),
            priority: 10,
            enabled: true,
            monthly_quota: Some(100_000),
            rate_limit_rpm: Some(60),
            cost_per_1k_tokens: Some(0.01),
            created_at: "2026-01-01".into(),
            updated_at: "2026-01-01".into(),
        };
        let ep = ExportProvider::from(&p);
        assert_eq!(ep.name, "Test");
        assert_eq!(ep.priority, 10);
        assert_eq!(ep.monthly_quota, Some(100_000));
    }

    #[test]
    fn empty_plaintext_round_trips() {
        let blob = encrypt_export(b"", "pw").expect("encrypt");
        let recovered = decrypt_export(&blob, "pw").expect("decrypt");
        assert!(recovered.is_empty());
    }

    #[test]
    fn large_payload_round_trips() {
        let data = vec![0xAB_u8; 100_000];
        let blob = encrypt_export(&data, "longpassword").expect("encrypt");
        let recovered = decrypt_export(&blob, "longpassword").expect("decrypt");
        assert_eq!(recovered.len(), 100_000);
        assert!(recovered.iter().all(|&b| b == 0xAB));
    }
}
