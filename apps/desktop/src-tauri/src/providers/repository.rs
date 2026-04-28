//! CRUD for the `providers` table.
//!
//! Encryption is enforced at the repository boundary: callers never
//! see ciphertext, repository never persists plaintext. New entries
//! get a UUID v4 id; updates and deletes are id-keyed.

use std::sync::Arc;

use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

use crate::crypto::{decrypt, encrypt, MasterKey, SecureStorageError};
use crate::db::{Database, DbError};

use super::model::{Provider, ProviderInput, ProviderKind};

/// Errors raised by [`ProviderRepository`] methods.
#[derive(thiserror::Error, Debug)]
pub enum RepositoryError {
    #[error(transparent)]
    Db(#[from] DbError),
    #[error("crypto failure: {0}")]
    Crypto(#[from] SecureStorageError),
    #[error("provider with id `{0}` not found")]
    NotFound(String),
    #[error("`kind` column has unknown value `{0}`; possibly forward-migrated database")]
    UnknownKind(String),
}

/// Persistence facade for providers. Holds a [`MasterKey`] so it can
/// encrypt/decrypt without re-reading the keyring on every call.
#[derive(Clone)]
pub struct ProviderRepository {
    db: Database,
    key: Arc<MasterKey>,
}

impl std::fmt::Debug for ProviderRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderRepository")
            .field("db_path", &self.db.path())
            .finish_non_exhaustive()
    }
}

impl ProviderRepository {
    /// Build a new repository. The master key is shared across the
    /// whole app via [`Arc`] — one secret loaded at boot, many
    /// repositories possible (in tests / future scoping).
    #[must_use]
    pub fn new(db: Database, key: Arc<MasterKey>) -> Self {
        Self { db, key }
    }

    /// Insert a new provider. Generates a UUID v4 id; returns the
    /// persisted [`Provider`] with `created_at` / `updated_at`
    /// populated by `SQLite`'s default value.
    pub fn add(&self, input: &ProviderInput) -> Result<Provider, RepositoryError> {
        let id = Uuid::new_v4().to_string();
        let ciphertext = encrypt(&self.key, input.api_key.as_bytes())?;
        let kind_str = input.kind.as_str();
        let enabled_int: i64 = i64::from(input.enabled);

        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO providers \
                 (id, name, kind, base_url, encrypted_api_key, priority, enabled) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    &id,
                    &input.name,
                    kind_str,
                    &input.base_url,
                    &ciphertext,
                    input.priority,
                    enabled_int,
                ],
            )?;
            Ok(())
        })?;

        self.get(&id)?.ok_or(RepositoryError::NotFound(id))
    }

    /// Mutate every column from `input`. Bumps `updated_at`. Returns
    /// the refreshed row.
    pub fn update(&self, id: &str, input: &ProviderInput) -> Result<Provider, RepositoryError> {
        let ciphertext = encrypt(&self.key, input.api_key.as_bytes())?;
        let kind_str = input.kind.as_str();
        let enabled_int: i64 = i64::from(input.enabled);

        let rows = self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE providers \
                 SET name=?1, kind=?2, base_url=?3, encrypted_api_key=?4, \
                     priority=?5, enabled=?6, updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') \
                 WHERE id=?7",
                params![
                    &input.name,
                    kind_str,
                    &input.base_url,
                    &ciphertext,
                    input.priority,
                    enabled_int,
                    id,
                ],
            )
        })?;
        if rows == 0 {
            return Err(RepositoryError::NotFound(id.to_owned()));
        }
        self.get(id)?
            .ok_or_else(|| RepositoryError::NotFound(id.to_owned()))
    }

    /// Remove by id. Errors if no row matched (vs a silent no-op,
    /// which would let UI bugs go unnoticed).
    pub fn delete(&self, id: &str) -> Result<(), RepositoryError> {
        let rows = self.db.with_connection(|conn| {
            conn.execute("DELETE FROM providers WHERE id=?1", params![id])
        })?;
        if rows == 0 {
            return Err(RepositoryError::NotFound(id.to_owned()));
        }
        Ok(())
    }

    /// Read one provider. Returns `None` if not found (callers that
    /// require a provider should compose with `.ok_or(NotFound)`).
    pub fn get(&self, id: &str) -> Result<Option<Provider>, RepositoryError> {
        let row: Option<ProviderRow> = self.db.with_connection(|conn| {
            conn.query_row(
                "SELECT id, name, kind, base_url, priority, enabled, created_at, updated_at \
                 FROM providers WHERE id=?1",
                params![id],
                ProviderRow::from_row,
            )
            .optional()
        })?;
        row.map(Provider::try_from).transpose()
    }

    /// Decrypt and return the API key for `id`. Kept off [`Provider`]
    /// so the plaintext never sits in a serialised JSON snapshot.
    pub fn get_decrypted_api_key(&self, id: &str) -> Result<String, RepositoryError> {
        let ciphertext: Vec<u8> = self
            .db
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT encrypted_api_key FROM providers WHERE id=?1",
                    params![id],
                    |row| row.get::<_, Vec<u8>>(0),
                )
                .optional()
            })?
            .ok_or_else(|| RepositoryError::NotFound(id.to_owned()))?;
        let plain = decrypt(&self.key, &ciphertext)?;
        Ok(String::from_utf8_lossy(&plain).into_owned())
    }

    /// List all providers, ordered by `SwitchEngine` priority
    /// (`enabled` first, then ascending priority).
    pub fn list(&self) -> Result<Vec<Provider>, RepositoryError> {
        let rows: Vec<ProviderRow> = self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, kind, base_url, priority, enabled, created_at, updated_at \
                 FROM providers \
                 ORDER BY enabled DESC, priority ASC, created_at ASC",
            )?;
            let iter = stmt.query_map([], ProviderRow::from_row)?;
            iter.collect::<Result<Vec<_>, _>>()
        })?;
        rows.into_iter().map(Provider::try_from).collect()
    }
}

/// Raw SQL row before the `kind` text → enum conversion.
struct ProviderRow {
    id: String,
    name: String,
    kind: String,
    base_url: String,
    priority: i32,
    enabled: i64,
    created_at: String,
    updated_at: String,
}

impl ProviderRow {
    fn from_row(row: &rusqlite::Row<'_>) -> Result<Self, rusqlite::Error> {
        Ok(Self {
            id: row.get(0)?,
            name: row.get(1)?,
            kind: row.get(2)?,
            base_url: row.get(3)?,
            priority: row.get(4)?,
            enabled: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        })
    }
}

impl TryFrom<ProviderRow> for Provider {
    type Error = RepositoryError;

    fn try_from(row: ProviderRow) -> Result<Self, Self::Error> {
        let kind = ProviderKind::parse(&row.kind).ok_or(RepositoryError::UnknownKind(row.kind))?;
        Ok(Self {
            id: row.id,
            name: row.name,
            kind,
            base_url: row.base_url,
            priority: row.priority,
            enabled: row.enabled != 0,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::MasterKey;
    use crate::db::{open_database, run_migrations};
    use tempfile::TempDir;

    /// Build a fresh repo backed by an ephemeral DB and a freshly
    /// generated master key. Returns `(_dir, repo)`; keep `_dir`
    /// alive for the duration of the test so the file isn't unlinked.
    fn fresh_repo() -> (TempDir, ProviderRepository) {
        let dir = TempDir::new().expect("tempdir");
        let db = open_database(dir.path().join("ccuse.db")).expect("open ok");
        run_migrations(&db).expect("migrate ok");
        let key = Arc::new(MasterKey::generate().expect("rng"));
        (dir, ProviderRepository::new(db, key))
    }

    fn sample_input() -> ProviderInput {
        ProviderInput {
            name: "Work OpenAI".to_owned(),
            kind: ProviderKind::Openai,
            base_url: "https://api.openai.com".to_owned(),
            api_key: "sk-real-secret-1234".to_owned(),
            priority: 50,
            enabled: true,
        }
    }

    #[test]
    fn add_and_get_round_trip() {
        let (_dir, repo) = fresh_repo();
        let saved = repo.add(&sample_input()).expect("add ok");
        assert!(!saved.id.is_empty());
        let fetched = repo
            .get(&saved.id)
            .expect("get ok")
            .expect("must be present");
        assert_eq!(fetched.name, "Work OpenAI");
        assert_eq!(fetched.kind, ProviderKind::Openai);
        assert_eq!(fetched.priority, 50);
        assert!(fetched.enabled);
    }

    #[test]
    fn api_key_is_encrypted_at_rest() {
        let (_dir, repo) = fresh_repo();
        let saved = repo.add(&sample_input()).expect("add ok");
        let stored: Vec<u8> = repo
            .db
            .with_connection(|c| {
                c.query_row(
                    "SELECT encrypted_api_key FROM providers WHERE id=?1",
                    params![&saved.id],
                    |r| r.get(0),
                )
            })
            .expect("query");
        // The plaintext must NOT appear in the persisted blob.
        let plaintext = b"sk-real-secret-1234";
        assert!(
            !stored.windows(plaintext.len()).any(|w| w == plaintext),
            "ciphertext must not contain plaintext key bytes",
        );
    }

    #[test]
    fn get_decrypted_api_key_returns_original_plaintext() {
        let (_dir, repo) = fresh_repo();
        let saved = repo.add(&sample_input()).expect("add ok");
        let plain = repo.get_decrypted_api_key(&saved.id).expect("decrypt ok");
        assert_eq!(plain, "sk-real-secret-1234");
    }

    #[test]
    fn list_orders_enabled_first_then_priority_asc() {
        let (_dir, repo) = fresh_repo();
        repo.add(&ProviderInput {
            name: "A".into(),
            priority: 100,
            enabled: false,
            ..sample_input()
        })
        .unwrap();
        repo.add(&ProviderInput {
            name: "B".into(),
            priority: 10,
            ..sample_input()
        })
        .unwrap();
        repo.add(&ProviderInput {
            name: "C".into(),
            priority: 5,
            ..sample_input()
        })
        .unwrap();
        let names: Vec<String> = repo
            .list()
            .expect("list")
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert_eq!(names, vec!["C".to_owned(), "B".into(), "A".into()]);
    }

    #[test]
    fn update_changes_fields_and_bumps_updated_at() {
        let (_dir, repo) = fresh_repo();
        let saved = repo.add(&sample_input()).expect("add");
        // Bump SQLite's clock by sleeping a millisecond — the
        // `strftime('%f')` granularity is enough.
        std::thread::sleep(std::time::Duration::from_millis(2));
        let next = ProviderInput {
            name: "Renamed".into(),
            priority: 1,
            api_key: "sk-rotated-secret".into(),
            ..sample_input()
        };
        let updated = repo.update(&saved.id, &next).expect("update");
        assert_eq!(updated.name, "Renamed");
        assert_eq!(updated.priority, 1);
        assert!(
            updated.updated_at >= saved.updated_at,
            "updated_at must not move backwards",
        );
        let plain = repo
            .get_decrypted_api_key(&saved.id)
            .expect("decrypt rotated");
        assert_eq!(plain, "sk-rotated-secret");
    }

    #[test]
    fn delete_removes_the_row() {
        let (_dir, repo) = fresh_repo();
        let saved = repo.add(&sample_input()).expect("add");
        repo.delete(&saved.id).expect("delete ok");
        assert!(repo.get(&saved.id).expect("get").is_none());
    }

    #[test]
    fn delete_unknown_id_returns_not_found() {
        let (_dir, repo) = fresh_repo();
        let err = repo.delete("does-not-exist").expect_err("must fail");
        assert!(matches!(err, RepositoryError::NotFound(_)));
    }

    #[test]
    fn update_unknown_id_returns_not_found() {
        let (_dir, repo) = fresh_repo();
        let err = repo
            .update("does-not-exist", &sample_input())
            .expect_err("must fail");
        assert!(matches!(err, RepositoryError::NotFound(_)));
    }

    #[test]
    fn unknown_kind_string_in_db_surfaces_as_unknown_kind_error() {
        let (_dir, repo) = fresh_repo();
        let saved = repo.add(&sample_input()).expect("add");
        // Manually corrupt the kind to simulate a forward-migrated db.
        repo.db
            .with_connection(|c| {
                c.execute(
                    "UPDATE providers SET kind='openrouter' WHERE id=?1",
                    params![&saved.id],
                )
            })
            .expect("corrupt");
        let err = repo.get(&saved.id).expect_err("must fail");
        assert!(matches!(err, RepositoryError::UnknownKind(s) if s == "openrouter"));
    }
}
