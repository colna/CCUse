//! T1.0.2.18 — [`RequestLogRepository`] for the `request_logs` table.
//!
//! Lightweight metadata log for every proxied request. Does NOT store
//! request/response bodies — only timing, token counts, and outcome.

use crate::db::{Database, DbError};

/// One row from `request_logs`.
#[derive(Debug, Clone)]
pub struct RequestLogEntry {
    pub id: i64,
    pub timestamp: String,
    pub provider_id: String,
    pub model: String,
    pub status: String,
    pub error_kind: Option<String>,
    pub latency_ms: i64,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub stream: bool,
}

/// Input for inserting a new request log.
#[derive(Debug, Clone)]
pub struct RequestLogInput {
    pub provider_id: String,
    pub model: String,
    pub status: String,
    pub error_kind: Option<String>,
    pub latency_ms: i64,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub stream: bool,
}

/// Repository for `request_logs` CRUD.
#[derive(Clone, Debug)]
pub struct RequestLogRepository {
    db: Database,
}

impl RequestLogRepository {
    /// Create a new repository backed by the given database.
    #[must_use]
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Record a request log entry.
    pub fn insert(&self, input: &RequestLogInput) -> Result<i64, DbError> {
        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO request_logs \
                 (provider_id, model, status, error_kind, latency_ms, \
                  prompt_tokens, completion_tokens, total_tokens, stream) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![
                    input.provider_id,
                    input.model,
                    input.status,
                    input.error_kind,
                    input.latency_ms,
                    input.prompt_tokens,
                    input.completion_tokens,
                    input.total_tokens,
                    i32::from(input.stream),
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// List the most recent `limit` entries, newest first.
    pub fn list_recent(&self, limit: u32) -> Result<Vec<RequestLogEntry>, DbError> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, timestamp, provider_id, model, status, error_kind, \
                 latency_ms, prompt_tokens, completion_tokens, total_tokens, stream \
                 FROM request_logs ORDER BY timestamp DESC LIMIT ?1",
            )?;
            let rows = stmt.query_map(rusqlite::params![limit], |row| {
                Ok(RequestLogEntry {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    provider_id: row.get(2)?,
                    model: row.get(3)?,
                    status: row.get(4)?,
                    error_kind: row.get(5)?,
                    latency_ms: row.get(6)?,
                    prompt_tokens: row.get(7)?,
                    completion_tokens: row.get(8)?,
                    total_tokens: row.get(9)?,
                    stream: row.get::<_, i32>(10)? != 0,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    /// List entries for a specific provider, newest first.
    pub fn list_by_provider(
        &self,
        provider_id: &str,
        limit: u32,
    ) -> Result<Vec<RequestLogEntry>, DbError> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, timestamp, provider_id, model, status, error_kind, \
                 latency_ms, prompt_tokens, completion_tokens, total_tokens, stream \
                 FROM request_logs WHERE provider_id = ?1 \
                 ORDER BY timestamp DESC LIMIT ?2",
            )?;
            let rows = stmt.query_map(rusqlite::params![provider_id, limit], |row| {
                Ok(RequestLogEntry {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    provider_id: row.get(2)?,
                    model: row.get(3)?,
                    status: row.get(4)?,
                    error_kind: row.get(5)?,
                    latency_ms: row.get(6)?,
                    prompt_tokens: row.get(7)?,
                    completion_tokens: row.get(8)?,
                    total_tokens: row.get(9)?,
                    stream: row.get::<_, i32>(10)? != 0,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    /// Count total entries.
    pub fn count(&self) -> Result<i64, DbError> {
        self.db.with_connection(|conn| {
            conn.query_row("SELECT COUNT(*) FROM request_logs", [], |r| r.get(0))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{open_database, run_migrations};
    use tempfile::TempDir;

    fn setup() -> RequestLogRepository {
        let dir = TempDir::new().expect("tempdir");
        let db = open_database(dir.path().join("test.db")).expect("open");
        run_migrations(&db).expect("migrate");
        // Insert test providers so FK constraints are satisfied.
        db.with_connection(|c| {
            for id in ["p1", "p2", "p3"] {
                c.execute(
                    "INSERT INTO providers (id, name, kind, base_url, encrypted_api_key) \
                     VALUES (?1, ?1, 'openai', 'https://api', x'00')",
                    rusqlite::params![id],
                )?;
            }
            Ok(())
        })
        .expect("seed providers");
        RequestLogRepository::new(db)
    }

    fn sample_input(provider: &str, ok: bool) -> RequestLogInput {
        RequestLogInput {
            provider_id: provider.into(),
            model: "gpt-4".into(),
            status: if ok { "ok".into() } else { "error".into() },
            error_kind: if ok { None } else { Some("Upstream".into()) },
            latency_ms: 150,
            prompt_tokens: Some(100),
            completion_tokens: Some(50),
            total_tokens: Some(150),
            stream: false,
        }
    }

    #[test]
    fn insert_and_list_round_trip() {
        let repo = setup();
        let id = repo.insert(&sample_input("p1", true)).expect("insert");
        assert!(id > 0);

        let entries = repo.list_recent(10).expect("list");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].provider_id, "p1");
        assert_eq!(entries[0].status, "ok");
        assert_eq!(entries[0].latency_ms, 150);
        assert!(!entries[0].stream);
    }

    #[test]
    fn list_by_provider_filters_correctly() {
        let repo = setup();
        repo.insert(&sample_input("p1", true)).expect("insert");
        repo.insert(&sample_input("p2", true)).expect("insert");
        repo.insert(&sample_input("p1", false)).expect("insert");

        let p1_entries = repo.list_by_provider("p1", 10).expect("list");
        assert_eq!(p1_entries.len(), 2);
        for entry in &p1_entries {
            assert_eq!(entry.provider_id, "p1");
        }
    }

    #[test]
    fn error_entry_stores_error_kind() {
        let repo = setup();
        repo.insert(&sample_input("p1", false)).expect("insert");
        let entries = repo.list_recent(1).expect("list");
        assert_eq!(entries[0].status, "error");
        assert_eq!(entries[0].error_kind.as_deref(), Some("Upstream"));
    }

    #[test]
    fn stream_flag_round_trips() {
        let repo = setup();
        let mut input = sample_input("p1", true);
        input.stream = true;
        repo.insert(&input).expect("insert");
        let entries = repo.list_recent(1).expect("list");
        assert!(entries[0].stream);
    }

    #[test]
    fn count_returns_correct_total() {
        let repo = setup();
        assert_eq!(repo.count().expect("count"), 0);
        repo.insert(&sample_input("p1", true)).expect("insert");
        repo.insert(&sample_input("p2", true)).expect("insert");
        assert_eq!(repo.count().expect("count"), 2);
    }
}
