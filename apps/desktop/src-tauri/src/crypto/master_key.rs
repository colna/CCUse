//! Master encryption key, persisted in the OS secret store.
//!
//! Behaviour:
//! 1. On first launch we generate a 256-bit key from the OS RNG and
//!    base64-encode it into the platform keyring (Keychain on macOS,
//!    Credential Manager on Windows, libsecret on Linux).
//! 2. Subsequent launches read it back. The plaintext key never
//!    touches disk in user-visible form.
//!
//! Tests don't depend on a real keyring — they pass an in-memory
//! [`KeyringBackend`] so they run unmodified in CI.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use rand::rngs::OsRng;
use rand::TryRngCore;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// `service` name registered with the OS keyring. Stable forever —
/// changing it would orphan every existing user's master key.
pub const KEYRING_SERVICE: &str = "io.ccuse.desktop";

/// `user` field within the keyring entry. The `_v1` suffix lets us
/// migrate to a new key shape later without colliding with the
/// existing entry.
pub const KEYRING_USER: &str = "master_key_v1";

/// AES-256 key length.
pub const MASTER_KEY_BYTES: usize = 32;

/// Error surface for master key operations.
#[derive(thiserror::Error, Debug)]
pub enum MasterKeyError {
    /// Reading or writing the keyring entry failed.
    #[error("keyring access failed: {0}")]
    Keyring(String),
    /// `OsRng` could not produce randomness — the OS entropy source
    /// is broken or seccomp-filtered.
    #[error("os random failure: {0}")]
    Random(String),
    /// Stored entry didn't decode as base64.
    #[error("stored master key is not valid base64: {0}")]
    Decode(String),
    /// Stored key has the wrong length (probably user tampering).
    #[error("stored master key is {found} bytes, expected {MASTER_KEY_BYTES}")]
    Length { found: usize },
}

/// Pluggable secret store. The default implementation talks to the
/// `keyring` crate; tests substitute an in-memory map.
pub trait KeyringBackend: Send + Sync {
    /// Read a `(service, user)` entry. `None` ⇒ entry doesn't exist
    /// (a "first launch" signal, not an error).
    fn get(&self, service: &str, user: &str) -> Result<Option<String>, MasterKeyError>;
    /// Write `(service, user) -> value`, creating the entry if it
    /// doesn't exist.
    fn set(&self, service: &str, user: &str, value: &str) -> Result<(), MasterKeyError>;
}

/// 32-byte secret. Drops with the bytes wiped so a heap dump after
/// shutdown doesn't leak the key.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct MasterKey([u8; MASTER_KEY_BYTES]);

impl MasterKey {
    /// Borrow the raw bytes — required by the AEAD layer.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; MASTER_KEY_BYTES] {
        &self.0
    }

    /// Generate a fresh key from the OS RNG. Surfaces the underlying
    /// error rather than panicking — `getrandom` can fail under
    /// seccomp / sandboxes.
    pub fn generate() -> Result<Self, MasterKeyError> {
        let mut buf = [0_u8; MASTER_KEY_BYTES];
        OsRng
            .try_fill_bytes(&mut buf)
            .map_err(|e| MasterKeyError::Random(e.to_string()))?;
        Ok(Self(buf))
    }
}

impl std::fmt::Debug for MasterKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("MasterKey").field(&"<redacted>").finish()
    }
}

/// Load the master key from `backend`, generating + storing a fresh
/// one if the entry doesn't exist yet.
pub fn load_or_create_master_key(
    backend: &dyn KeyringBackend,
) -> Result<MasterKey, MasterKeyError> {
    if let Some(raw) = backend.get(KEYRING_SERVICE, KEYRING_USER)? {
        let mut decoded = B64
            .decode(raw.as_bytes())
            .map_err(|e| MasterKeyError::Decode(e.to_string()))?;
        if decoded.len() != MASTER_KEY_BYTES {
            let len = decoded.len();
            decoded.zeroize();
            return Err(MasterKeyError::Length { found: len });
        }
        let mut key = [0_u8; MASTER_KEY_BYTES];
        key.copy_from_slice(&decoded);
        decoded.zeroize();
        return Ok(MasterKey(key));
    }
    let key = MasterKey::generate()?;
    let encoded = B64.encode(key.as_bytes());
    backend.set(KEYRING_SERVICE, KEYRING_USER, &encoded)?;
    Ok(key)
}

/// Production backend backed by the `keyring` crate. Constructed
/// once at app start and handed to [`load_or_create_master_key`].
pub struct OsKeyringBackend;

impl KeyringBackend for OsKeyringBackend {
    fn get(&self, service: &str, user: &str) -> Result<Option<String>, MasterKeyError> {
        let entry = keyring::Entry::new(service, user)
            .map_err(|e| MasterKeyError::Keyring(e.to_string()))?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(other) => Err(MasterKeyError::Keyring(other.to_string())),
        }
    }

    fn set(&self, service: &str, user: &str, value: &str) -> Result<(), MasterKeyError> {
        let entry = keyring::Entry::new(service, user)
            .map_err(|e| MasterKeyError::Keyring(e.to_string()))?;
        entry
            .set_password(value)
            .map_err(|e| MasterKeyError::Keyring(e.to_string()))
    }
}

impl std::fmt::Debug for OsKeyringBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OsKeyringBackend").finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// In-memory backend for tests. Stores under the same
    /// `(service, user)` shape so any production-side bug in the
    /// keying logic surfaces here too.
    #[derive(Default)]
    struct MemoryBackend {
        store: Mutex<HashMap<(String, String), String>>,
    }

    impl KeyringBackend for MemoryBackend {
        fn get(&self, service: &str, user: &str) -> Result<Option<String>, MasterKeyError> {
            Ok(self
                .store
                .lock()
                .unwrap()
                .get(&(service.to_owned(), user.to_owned()))
                .cloned())
        }

        fn set(&self, service: &str, user: &str, value: &str) -> Result<(), MasterKeyError> {
            self.store
                .lock()
                .unwrap()
                .insert((service.to_owned(), user.to_owned()), value.to_owned());
            Ok(())
        }
    }

    #[test]
    fn first_launch_generates_and_persists_a_fresh_key() {
        let backend = MemoryBackend::default();
        let key = load_or_create_master_key(&backend).expect("load ok");
        assert_eq!(key.as_bytes().len(), MASTER_KEY_BYTES);
        let stored = backend
            .get(KEYRING_SERVICE, KEYRING_USER)
            .expect("get ok")
            .expect("entry must be present after first launch");
        let decoded = B64.decode(stored).expect("base64");
        assert_eq!(decoded, key.as_bytes());
    }

    #[test]
    fn second_launch_returns_the_same_key() {
        let backend = MemoryBackend::default();
        let first = load_or_create_master_key(&backend).expect("first load");
        let second = load_or_create_master_key(&backend).expect("second load");
        assert_eq!(first.as_bytes(), second.as_bytes());
    }

    #[test]
    fn stored_key_with_wrong_length_yields_length_error() {
        let backend = MemoryBackend::default();
        // 16 bytes of zeros, base64-encoded.
        let bad = B64.encode([0_u8; 16]);
        backend
            .set(KEYRING_SERVICE, KEYRING_USER, &bad)
            .expect("set ok");
        let err = load_or_create_master_key(&backend).expect_err("must reject short key");
        match err {
            MasterKeyError::Length { found } => assert_eq!(found, 16),
            other => panic!("expected Length error, got {other:?}"),
        }
    }

    #[test]
    fn invalid_base64_in_store_surfaces_decode_error() {
        let backend = MemoryBackend::default();
        backend
            .set(KEYRING_SERVICE, KEYRING_USER, "not base64 !!!")
            .expect("set ok");
        let err = load_or_create_master_key(&backend).expect_err("must reject");
        assert!(matches!(err, MasterKeyError::Decode(_)));
    }

    #[test]
    fn debug_does_not_leak_key_material() {
        let key = MasterKey::generate().expect("gen ok");
        let rendered = format!("{key:?}");
        assert!(
            rendered.contains("redacted"),
            "Debug must mask key material, got `{rendered}`",
        );
        assert!(!rendered.contains(&format!("{:?}", key.as_bytes())));
    }

    #[test]
    fn keyring_service_and_user_constants_are_stable() {
        // These values are part of the *user's* persisted state —
        // changing them silently would orphan their existing master
        // key. Pin them at the test layer so the bump is loud.
        assert_eq!(KEYRING_SERVICE, "io.ccuse.desktop");
        assert_eq!(KEYRING_USER, "master_key_v1");
    }
}
