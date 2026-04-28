//! Forward-only schema migrations for the local database.
//!
//! Pinned model:
//! * Each migration has a numeric `version` and an `IMMEDIATE` SQL
//!   blob embedded via `include_str!` (so a release binary is
//!   self-sufficient — no migrations directory shipped).
//! * The `_migrations` table records `(version, applied_at)`; we
//!   replay every embedded migration whose version is greater than
//!   `MAX(version)`.
//! * Each migration runs inside a transaction; a failure rolls back
//!   the partial schema before the version row is recorded.

use rusqlite::{Connection, Transaction};

use super::init::DbError;

/// One migration's static description.
struct Migration {
    version: u32,
    name: &'static str,
    sql: &'static str,
}

/// Embedded migrations, append-only and ordered by version.
///
/// New migrations land at the end and never modify earlier entries —
/// a deployed binary may have applied N already and only see new
/// entries on next launch.
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "init",
        sql: include_str!("../../migrations/0001_init.sql"),
    },
    Migration {
        version: 2,
        name: "switch_history",
        sql: include_str!("../../migrations/0002_switch_history.sql"),
    },
    Migration {
        version: 3,
        name: "request_logs",
        sql: include_str!("../../migrations/0003_request_logs.sql"),
    },
    Migration {
        version: 4,
        name: "provider_quota",
        sql: include_str!("../../migrations/0004_provider_quota.sql"),
    },
];

/// Apply every migration whose version is newer than the highest
/// already recorded in `_migrations`. Idempotent — calling twice is
/// a no-op the second time.
pub fn run_migrations(db: &super::Database) -> Result<u32, DbError> {
    db.with_connection(|conn| {
        ensure_migrations_table(conn)?;
        let already: u32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM _migrations",
                [],
                |row| row.get::<_, u32>(0),
            )
            .unwrap_or(0);
        let mut applied = already;
        for migration in MIGRATIONS.iter().filter(|m| m.version > already) {
            apply(conn, migration)?;
            applied = migration.version;
        }
        Ok(applied)
    })
}

fn ensure_migrations_table(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _migrations (
            version    INTEGER PRIMARY KEY,
            name       TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );",
    )
}

fn apply(conn: &mut Connection, migration: &Migration) -> Result<(), rusqlite::Error> {
    let tx: Transaction<'_> = conn.transaction()?;
    tx.execute_batch(migration.sql)?;
    tx.execute(
        "INSERT INTO _migrations (version, name) VALUES (?1, ?2)",
        rusqlite::params![migration.version, migration.name],
    )?;
    tx.commit()
}

#[cfg(test)]
mod tests {
    use super::super::init::open_database;
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn fresh_database_applies_all_embedded_migrations() {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("fresh.db");
        let db = open_database(&path).expect("open ok");

        let highest = run_migrations(&db).expect("migrate ok");
        assert_eq!(highest, MIGRATIONS.last().expect("at least one").version);
    }

    #[test]
    fn migrations_create_expected_tables() {
        let dir = TempDir::new().expect("tempdir");
        let db = open_database(dir.path().join("schema.db")).expect("open ok");
        run_migrations(&db).expect("migrate ok");

        for table in [
            "providers",
            "app_config",
            "switch_history",
            "request_logs",
            "_migrations",
        ] {
            let count: i64 = db
                .with_connection(|c| {
                    c.query_row(
                        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                        rusqlite::params![table],
                        |r| r.get(0),
                    )
                })
                .expect("schema query");
            assert_eq!(count, 1, "table `{table}` must exist");
        }
    }

    #[test]
    fn second_call_is_a_no_op() {
        let dir = TempDir::new().expect("tempdir");
        let db = open_database(dir.path().join("idempotent.db")).expect("open ok");

        let first = run_migrations(&db).expect("first migrate");
        let second = run_migrations(&db).expect("second migrate");
        assert_eq!(first, second, "second invocation must report same version");

        let row_count: i64 = db
            .with_connection(|c| c.query_row("SELECT COUNT(*) FROM _migrations", [], |r| r.get(0)))
            .expect("count");
        assert_eq!(
            row_count,
            i64::try_from(MIGRATIONS.len()).expect("fits"),
            "only one row per migration",
        );
    }

    #[test]
    fn providers_priority_index_exists() {
        let dir = TempDir::new().expect("tempdir");
        let db = open_database(dir.path().join("idx.db")).expect("open ok");
        run_migrations(&db).expect("migrate ok");
        let count: i64 = db
            .with_connection(|c| {
                c.query_row(
                    "SELECT COUNT(*) FROM sqlite_master \
                     WHERE type='index' AND name='providers_priority_idx'",
                    [],
                    |r| r.get(0),
                )
            })
            .expect("schema query");
        assert_eq!(count, 1);
    }

    #[test]
    fn enabled_check_constraint_rejects_invalid_values() {
        let dir = TempDir::new().expect("tempdir");
        let db = open_database(dir.path().join("check.db")).expect("open ok");
        run_migrations(&db).expect("migrate ok");
        let outcome = db.with_connection(|c| {
            c.execute(
                "INSERT INTO providers (id, name, kind, base_url, encrypted_api_key, enabled) \
                 VALUES ('p1', 'p', 'openai', 'https://api.openai.com', x'00', 2)",
                [],
            )
        });
        assert!(outcome.is_err(), "CHECK (enabled IN (0,1)) should reject 2");
    }
}
