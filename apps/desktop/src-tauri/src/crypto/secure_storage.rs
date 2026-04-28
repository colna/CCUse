//! AES-256-GCM envelope around the master key from T1.0.1.16.
//!
//! Wire layout written into `providers.encrypted_api_key`:
//!
//! ```text
//!   1 byte    version  (0x01)
//!  12 bytes   nonce    (random per call, never reused)
//!   N bytes   ciphertext (plaintext.len() + 16 tag)
//! ```
//!
//! GCM authenticates ciphertext + (empty) AAD, so any tamper /
//! truncation surfaces as `decrypt → MasterKeyError::Aead`.

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use rand::rngs::OsRng;
use rand::TryRngCore;

use super::master_key::MasterKey;

/// Wire-format version byte. Bump when the envelope shape changes
/// so old ciphertext can still be decrypted alongside new entries.
pub const ENVELOPE_VERSION: u8 = 0x01;
/// GCM nonce length, in bytes (96 bits — the standard).
pub const NONCE_LEN: usize = 12;

/// Errors produced by encrypt / decrypt.
#[derive(thiserror::Error, Debug)]
pub enum SecureStorageError {
    /// `OsRng` failed to fill the nonce buffer.
    #[error("os random failure: {0}")]
    Random(String),
    /// AEAD seal/open returned an error (almost always = wrong key,
    /// truncation, or tampering).
    #[error("aead operation failed (wrong key or tampered ciphertext)")]
    Aead,
    /// Ciphertext is shorter than `version + nonce + tag`.
    #[error("ciphertext too short to be a valid envelope ({0} bytes)")]
    Truncated(usize),
    /// Ciphertext starts with an unknown version byte.
    #[error("unsupported envelope version: 0x{0:02x}")]
    UnsupportedVersion(u8),
}

/// Wrap `plaintext` under `key`. Returns the wire-format envelope.
///
/// Generates a fresh random nonce per call; nonce reuse with the
/// same key trivially leaks plaintext XOR — the OS RNG is non-
/// negotiable here.
pub fn encrypt(key: &MasterKey, plaintext: &[u8]) -> Result<Vec<u8>, SecureStorageError> {
    let cipher = Aes256Gcm::new_from_slice(key.as_bytes()).expect("MasterKey is 32 bytes");
    let mut nonce_bytes = [0_u8; NONCE_LEN];
    OsRng
        .try_fill_bytes(&mut nonce_bytes)
        .map_err(|e| SecureStorageError::Random(e.to_string()))?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| SecureStorageError::Aead)?;

    let mut out = Vec::with_capacity(1 + NONCE_LEN + ciphertext.len());
    out.push(ENVELOPE_VERSION);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Reverse of [`encrypt`]. Returns the original plaintext bytes;
/// any tamper / wrong-key / truncation surfaces as `Err`.
pub fn decrypt(key: &MasterKey, envelope: &[u8]) -> Result<Vec<u8>, SecureStorageError> {
    if envelope.len() < 1 + NONCE_LEN + 16 {
        return Err(SecureStorageError::Truncated(envelope.len()));
    }
    let version = envelope[0];
    if version != ENVELOPE_VERSION {
        return Err(SecureStorageError::UnsupportedVersion(version));
    }
    let nonce = Nonce::from_slice(&envelope[1..=NONCE_LEN]);
    let ciphertext = &envelope[1 + NONCE_LEN..];

    let cipher = Aes256Gcm::new_from_slice(key.as_bytes()).expect("MasterKey is 32 bytes");
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| SecureStorageError::Aead)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_key() -> MasterKey {
        MasterKey::generate().expect("rng ok")
    }

    #[test]
    fn round_trip_recovers_plaintext() {
        let key = fresh_key();
        let envelope = encrypt(&key, b"sk-xxxx-secret").expect("encrypt");
        let recovered = decrypt(&key, &envelope).expect("decrypt");
        assert_eq!(recovered, b"sk-xxxx-secret");
    }

    #[test]
    fn envelope_starts_with_version_byte() {
        let key = fresh_key();
        let envelope = encrypt(&key, b"hello").expect("encrypt");
        assert_eq!(envelope[0], ENVELOPE_VERSION);
    }

    #[test]
    fn nonce_is_unique_across_repeated_calls_with_same_input() {
        let key = fresh_key();
        let a = encrypt(&key, b"same input").expect("encrypt a");
        let b = encrypt(&key, b"same input").expect("encrypt b");
        // 12 nonce bytes must differ; ciphertext therefore differs too.
        assert_ne!(&a[1..=NONCE_LEN], &b[1..=NONCE_LEN]);
        assert_ne!(a, b);
    }

    #[test]
    fn decryption_with_wrong_key_fails() {
        let envelope = encrypt(&fresh_key(), b"top secret").expect("encrypt");
        let other = fresh_key();
        let err = decrypt(&other, &envelope).expect_err("must fail");
        assert!(matches!(err, SecureStorageError::Aead));
    }

    #[test]
    fn flipped_ciphertext_byte_is_detected() {
        let key = fresh_key();
        let mut envelope = encrypt(&key, b"detect me").expect("encrypt");
        // Flip a byte in the tag region (tail).
        let last = envelope.len() - 1;
        envelope[last] ^= 0x01;
        let err = decrypt(&key, &envelope).expect_err("must fail");
        assert!(matches!(err, SecureStorageError::Aead));
    }

    #[test]
    fn truncated_envelope_yields_truncated_error() {
        let key = fresh_key();
        let envelope = encrypt(&key, b"short").expect("encrypt");
        let cut = &envelope[..10];
        let err = decrypt(&key, cut).expect_err("must fail");
        assert!(matches!(err, SecureStorageError::Truncated(10)));
    }

    #[test]
    fn unknown_version_byte_is_rejected() {
        let key = fresh_key();
        let mut envelope = encrypt(&key, b"hi").expect("encrypt");
        envelope[0] = 0xFF;
        let err = decrypt(&key, &envelope).expect_err("must fail");
        assert!(matches!(err, SecureStorageError::UnsupportedVersion(0xFF)));
    }

    #[test]
    fn empty_plaintext_round_trips() {
        let key = fresh_key();
        let envelope = encrypt(&key, b"").expect("encrypt");
        let plain = decrypt(&key, &envelope).expect("decrypt");
        assert!(plain.is_empty());
    }
}
