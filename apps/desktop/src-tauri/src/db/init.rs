//! Open and initialise the application's `SQLite` database.
//!
//! Pinned PRAGMAs:
//! * `journal_mode = WAL` — concurrent readers + single writer; the
//!   default rollback journal blocks readers during writes.
//! * `foreign_keys = ON` — `SQLite` ships them off; we always want them.
//! * `synchronous = NORMAL` — WAL-safe trade-off (durable on commit,
//!   not on every fsync).
//!
//! On Unix the file is `chmod 0600` so other local users cannot read
//! the encrypted provider blobs that land here in T1.0.1.17–18.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rusqlite::Connection;

/// Errors raised while opening or preparing the database file.
#[derive(thiserror::Error, Debug)]
pub enum DbError {
    /// Failed to create the parent directory for the database file.
    #[error("failed to create database directory `{path}`: {source}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// `rusqlite::Connection::open` returned an error.
    #[error("failed to open database at `{path}`: {source}")]
    Open {
        path: PathBuf,
        #[source]
        source: rusqlite::Error,
    },

    /// A `PRAGMA` or `SELECT` issued during initialisation failed.
    #[error("failed to configure database at `{path}`: {source}")]
    Configure {
        path: PathBuf,
        #[source]
        source: rusqlite::Error,
    },

    /// Failed to set 0600 permissions on the database file (Unix only).
    #[cfg(unix)]
    #[error("failed to chmod database file `{path}` to 0600: {source}")]
    Chmod {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Thread-safe handle to the application database.
///
/// `rusqlite::Connection` is not `Sync`, so we wrap it in a `Mutex`.
/// The handle itself is cheap to `clone` because it's `Arc`-based —
/// the `Tauri` state layer keeps one per app and clones into commands.
#[derive(Clone)]
pub struct Database {
    path: PathBuf,
    conn: Arc<Mutex<Connection>>,
}

impl std::fmt::Debug for Database {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Database")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl Database {
    /// Filesystem path the database is backed by.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Run `f` against an exclusive borrow of the underlying
    /// connection. The mutex is poisoned-safe — we map a poisoned
    /// lock back to a domain error rather than panicking.
    pub fn with_connection<F, T>(&self, f: F) -> Result<T, DbError>
    where
        F: FnOnce(&mut Connection) -> Result<T, rusqlite::Error>,
    {
        let mut guard = self.conn.lock().map_err(|_| DbError::Configure {
            path: self.path.clone(),
            source: rusqlite::Error::InvalidQuery,
        })?;
        f(&mut guard).map_err(|source| DbError::Configure {
            path: self.path.clone(),
            source,
        })
    }
}

/// Open `path`, applying `CCUse`'s standard configuration. Creates
/// parent directories if missing and locks down file perms on Unix.
pub fn open_database(path: impl AsRef<Path>) -> Result<Database, DbError> {
    let path = path.as_ref().to_path_buf();
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|source| DbError::CreateDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }
    }

    let conn = Connection::open(&path).map_err(|source| DbError::Open {
        path: path.clone(),
        source,
    })?;

    apply_pragmas(&conn, &path)?;

    #[cfg(unix)]
    apply_unix_perms(&path)?;

    Ok(Database {
        path,
        conn: Arc::new(Mutex::new(conn)),
    })
}

/// Set the PRAGMAs that every `CCUse` session expects.
///
/// `journal_mode = WAL` is queried via `pragma_query_value` because
/// `SQLite` returns the resulting mode (e.g. `"wal"`); we treat
/// anything else as a configure failure rather than silently
/// continuing in rollback-journal mode.
fn apply_pragmas(conn: &Connection, path: &Path) -> Result<(), DbError> {
    let map_err = |source: rusqlite::Error| DbError::Configure {
        path: path.to_path_buf(),
        source,
    };

    let mode: String = conn
        .pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))
        .map_err(map_err)?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(DbError::Configure {
            path: path.to_path_buf(),
            // SQLITE returned a different mode; surface as InvalidQuery
            // so the call site sees "configuration rejected" without us
            // inventing a new variant for this edge case.
            source: rusqlite::Error::InvalidQuery,
        });
    }

    conn.pragma_update(None, "foreign_keys", true)
        .map_err(map_err)?;
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(map_err)?;

    Ok(())
}

#[cfg(unix)]
fn apply_unix_perms(path: &Path) -> Result<(), DbError> {
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(path, perms).map_err(|source| DbError::Chmod {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn db_path(dir: &TempDir, file: &str) -> PathBuf {
        dir.path().join(file)
    }

    #[test]
    fn open_database_creates_parent_directories() {
        let dir = TempDir::new().expect("tempdir");
        let nested = dir.path().join("nested/sub/ccuse.db");
        let _db = open_database(&nested).expect("open should succeed and create parents");
        assert!(nested.exists(), "database file must be created");
        assert!(
            nested.parent().expect("parent").exists(),
            "parent dir must be created",
        );
    }

    #[test]
    fn open_database_enables_wal_mode() {
        let dir = TempDir::new().expect("tempdir");
        let path = db_path(&dir, "wal.db");
        let db = open_database(&path).expect("open ok");
        let mode: String = db
            .with_connection(|c| c.query_row("PRAGMA journal_mode;", [], |r| r.get(0)))
            .expect("pragma read");
        assert_eq!(mode.to_lowercase(), "wal");
    }

    #[test]
    fn open_database_enables_foreign_keys() {
        let dir = TempDir::new().expect("tempdir");
        let path = db_path(&dir, "fk.db");
        let db = open_database(&path).expect("open ok");
        let on: i64 = db
            .with_connection(|c| c.query_row("PRAGMA foreign_keys;", [], |r| r.get(0)))
            .expect("pragma read");
        assert_eq!(on, 1, "foreign_keys must be ON");
    }

    #[test]
    fn open_database_sets_synchronous_normal() {
        let dir = TempDir::new().expect("tempdir");
        let path = db_path(&dir, "sync.db");
        let db = open_database(&path).expect("open ok");
        let level: i64 = db
            .with_connection(|c| c.query_row("PRAGMA synchronous;", [], |r| r.get(0)))
            .expect("pragma read");
        // 1 = NORMAL per SQLite documentation.
        assert_eq!(level, 1);
    }

    #[cfg(unix)]
    #[test]
    fn open_database_locks_file_to_0600_on_unix() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new().expect("tempdir");
        let path = db_path(&dir, "perms.db");
        let _db = open_database(&path).expect("open ok");
        let mode = std::fs::metadata(&path).expect("stat").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "owner-only read/write");
    }

    #[test]
    fn database_path_returns_what_was_opened() {
        let dir = TempDir::new().expect("tempdir");
        let path = db_path(&dir, "path.db");
        let db = open_database(&path).expect("open ok");
        assert_eq!(db.path(), path.as_path());
    }
}
