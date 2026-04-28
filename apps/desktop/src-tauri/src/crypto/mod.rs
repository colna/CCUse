//! Cryptography building blocks.
//!
//! Phase 1.0.1 ships only the master key (T1.0.1.16). The AEAD layer
//! that encrypts provider API keys lives in T1.0.1.17 and reuses the
//! 32-byte secret loaded here.

pub mod master_key;
pub mod secure_storage;

pub use master_key::{
    load_or_create_master_key, KeyringBackend, MasterKey, MasterKeyError, KEYRING_SERVICE,
    KEYRING_USER, MASTER_KEY_BYTES,
};
pub use secure_storage::{decrypt, encrypt, SecureStorageError, ENVELOPE_VERSION, NONCE_LEN};
