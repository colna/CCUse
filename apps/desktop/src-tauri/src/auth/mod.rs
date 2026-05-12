//! Local authentication primitives.
//!
//! Phase 1.0.1 only ships [`local_key`] — the `sk-local-…` API token the
//! desktop app gives every client that wants to talk to the local proxy.
//! T1.0.1.13 layers a constant-time validation middleware on top, and
//! T1.0.1.16 introduces the master key for at-rest secrets.

pub mod local_key;
pub mod middleware;

pub use local_key::{generate_local_api_key, LocalApiKey, LOCAL_API_KEY_PREFIX, RANDOM_PART_LEN};
pub use middleware::{key_store, require_local_api_key, KeyStore, LocalApiKeySet};
