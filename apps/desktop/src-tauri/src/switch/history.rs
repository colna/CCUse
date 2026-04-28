//! T1.0.2.17 — [`SwitchHistoryRepository`] for the `switch_history` table.
//!
//! Records every provider switch event for audit / debugging.

use crate::db::{Database, DbError};

/// One row from `switch_history`.
#[derive(Debug, Clone)]
pub struct SwitchHistoryEntry {
    pub id: i64,
    pub timestamp: String,
    pub from_provider: Option<String>,
    pub to_provider: String,
    pub strategy: String,
    pub reason: String,
    pub attempts: i32,
}

/// Input for inserting a new switch event.
#[derive(Debug, Clone)]
pub struct SwitchHistoryInput {
    pub from_provider: Option<String>,
    pub to_provider: String,
    pub strategy: String,
    pub reason: String,
    pub attempts: i32,
}

/// Repository for `switch_history` CRUD.
#[derive(Clone, Debug)]
pub struct SwitchHistoryRepository {
    db: Database,
}

impl SwitchHistoryRepository {
    /// Create a new repository backed by the given database.
    #[must_use]
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Record a switch event.
    pub fn insert(&self, input: &SwitchHistoryInput) -> Result<i64, DbError> {
        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO switch_history (from_provider, to_provider, strategy, reason, attempts) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    input.from_provider,
                    input.to_provider,
                    input.strategy,
                    input.reason,
                    input.attempts,
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// List the most recent `limit` entries, newest first.
    pub fn list_recent(&self, limit: u32) -> Result<Vec<SwitchHistoryEntry>, DbError> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, timestamp, from_provider, to_provider, strategy, reason, attempts \
                 FROM switch_history ORDER BY timestamp DESC LIMIT ?1",
            )?;
            let rows = stmt.query_map(rusqlite::params![limit], |row| {
                Ok(SwitchHistoryEntry {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    from_provider: row.get(2)?,
                    to_provider: row.get(3)?,
                    strategy: row.get(4)?,
                    reason: row.get(5)?,
                    attempts: row.get(6)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    /// Count total entries (useful for pagination / stats).
    pub fn count(&self) -> Result<i64, DbError> {
        self.db.with_connection(|conn| {
            conn.query_row("SELECT COUNT(*) FROM switch_history", [], |r| r.get(0))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{open_database, run_migrations};
    use tempfile::TempDir;

    fn setup() -> SwitchHistoryRepository {
        let dir = TempDir::new().expect("tempdir");
        let db = open_database(dir.path().join("test.db")).expect("open");
        run_migrations(&db).expect("migrate");
        // Insert test providers so FK constraints are satisfied.
        db.with_connection(|c| {
            for id in ["p1", "p2", "p3", "p4"] {
                c.execute(
                    "INSERT INTO providers (id, name, kind, base_url, encrypted_api_key) \
                     VALUES (?1, ?1, 'openai', 'https://api', x'00')",
                    rusqlite::params![id],
                )?;
            }
            Ok(())
        })
        .expect("seed providers");
        SwitchHistoryRepository::new(db)
    }

    #[test]
    fn insert_and_list_round_trip() {
        let repo = setup();
        let input = SwitchHistoryInput {
            from_provider: Some("p1".into()),
            to_provider: "p2".into(),
            strategy: "priority".into(),
            reason: "upstream_500".into(),
            attempts: 2,
        };
        let id = repo.insert(&input).expect("insert");
        assert!(id > 0);

        let entries = repo.list_recent(10).expect("list");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].to_provider, "p2");
        assert_eq!(entries[0].attempts, 2);
    }

    #[test]
    fn list_recent_respects_limit() {
        let repo = setup();
        for i in 1..=4 {
            repo.insert(&SwitchHistoryInput {
                from_provider: None,
                to_provider: format!("p{i}"),
                strategy: "priority".into(),
                reason: "test".into(),
                attempts: 1,
            })
            .expect("insert");
        }
        let entries = repo.list_recent(3).expect("list");
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn count_returns_correct_total() {
        let repo = setup();
        assert_eq!(repo.count().expect("count"), 0);
        repo.insert(&SwitchHistoryInput {
            from_provider: None,
            to_provider: "p1".into(),
            strategy: "smart".into(),
            reason: "manual".into(),
            attempts: 1,
        })
        .expect("insert");
        assert_eq!(repo.count().expect("count"), 1);
    }

    #[test]
    fn from_provider_can_be_null() {
        let repo = setup();
        let id = repo
            .insert(&SwitchHistoryInput {
                from_provider: None,
                to_provider: "p1".into(),
                strategy: "priority".into(),
                reason: "first_dispatch".into(),
                attempts: 1,
            })
            .expect("insert");
        let entries = repo.list_recent(1).expect("list");
        assert_eq!(entries[0].id, id);
        assert!(entries[0].from_provider.is_none());
    }
}
