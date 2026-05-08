//! T1.0.4.14 — Aggregation query commands for the monitoring dashboard.
//!
//! Provides time-bucket aggregated metrics, cost summaries, and switch
//! event timelines for the frontend dashboard charts.

use serde::Serialize;
use tauri::State;

use crate::db::Database;

const DELETED_PROVIDER_LABEL: &str = "[deleted provider]";

/// One time-bucket row for charts (5-minute buckets).
#[derive(Debug, Clone, Serialize)]
pub struct MetricsBucket {
    pub timestamp: String,
    pub request_count: i64,
    pub success_count: i64,
    pub success_rate: f64,
    pub avg_latency_ms: f64,
    pub p95_latency_ms: i64,
}

/// Cost summary per provider.
#[derive(Debug, Clone, Serialize)]
pub struct ProviderCostSummary {
    pub provider_id: String,
    pub provider_name: String,
    pub total_tokens: i64,
    pub total_cost: f64,
    pub request_count: i64,
}

/// Switch event for the timeline.
#[derive(Debug, Clone, Serialize)]
pub struct SwitchEvent {
    pub id: i64,
    pub timestamp: String,
    pub from_provider: Option<String>,
    pub to_provider: String,
    pub strategy: String,
    pub reason: String,
    pub attempts: i32,
}

/// Return 24h of metrics in 5-minute time buckets.
#[tauri::command]
pub async fn get_metrics_timeseries(db: State<'_, Database>) -> Result<Vec<MetricsBucket>, String> {
    query_metrics_timeseries(&db)
}

pub fn query_metrics_timeseries(db: &Database) -> Result<Vec<MetricsBucket>, String> {
    db.with_connection(|conn| {
        let mut stmt = conn.prepare(
            "WITH buckets AS ( \
               SELECT \
                 strftime('%Y-%m-%dT%H:', timestamp) || \
                   printf('%02d', (CAST(strftime('%M', timestamp) AS INTEGER) / 5) * 5) \
                   || ':00Z' AS bucket, \
                 status, \
                 latency_ms \
               FROM request_logs \
               WHERE timestamp >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-24 hours') \
             ) \
             SELECT \
               bucket, \
               COUNT(*) AS request_count, \
               SUM(CASE WHEN status = 'ok' THEN 1 ELSE 0 END) AS success_count, \
               ROUND(1.0 * SUM(CASE WHEN status = 'ok' THEN 1 ELSE 0 END) / COUNT(*), 4) \
                 AS success_rate, \
               ROUND(AVG(latency_ms), 1) AS avg_latency_ms, \
               MAX(latency_ms) AS p95_latency_ms \
             FROM buckets \
             GROUP BY bucket \
             ORDER BY bucket ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(MetricsBucket {
                timestamp: row.get(0)?,
                request_count: row.get(1)?,
                success_count: row.get(2)?,
                success_rate: row.get(3)?,
                avg_latency_ms: row.get(4)?,
                p95_latency_ms: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
    })
    .map_err(|e| e.to_string())
}

/// Return cost (token count) summary grouped by provider.
#[tauri::command]
pub async fn get_provider_cost_summary(
    db: State<'_, Database>,
) -> Result<Vec<ProviderCostSummary>, String> {
    query_provider_cost_summary(&db)
}

pub fn query_provider_cost_summary(db: &Database) -> Result<Vec<ProviderCostSummary>, String> {
    db.with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT p.id, \
                    p.name, \
                    COALESCE(SUM(total_tokens), 0) AS total_tokens, \
                    ROUND(COALESCE(SUM( \
                        COALESCE(total_tokens, 0) * COALESCE(p.cost_per_1k_tokens, 0.0) / 1000.0 \
                    ), 0.0), 6) AS total_cost, \
                    COUNT(request_logs.id) AS request_count \
             FROM request_logs \
             JOIN providers p ON p.id = request_logs.provider_id \
             WHERE request_logs.timestamp >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-24 hours') \
             GROUP BY p.id, p.name \
             ORDER BY total_cost DESC, request_count DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ProviderCostSummary {
                provider_id: row.get(0)?,
                provider_name: row.get(1)?,
                total_tokens: row.get(2)?,
                total_cost: row.get(3)?,
                request_count: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
    })
    .map_err(|e| e.to_string())
}

/// Return recent switch events for the timeline.
#[tauri::command]
pub async fn get_switch_timeline(db: State<'_, Database>) -> Result<Vec<SwitchEvent>, String> {
    query_switch_timeline(&db)
}

pub fn query_switch_timeline(db: &Database) -> Result<Vec<SwitchEvent>, String> {
    db.with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, from_provider, COALESCE(to_provider, ?1), \
                    strategy, reason, attempts \
             FROM switch_history \
             ORDER BY timestamp DESC \
             LIMIT 50",
        )?;
        let rows = stmt.query_map([DELETED_PROVIDER_LABEL], |row| {
            Ok(SwitchEvent {
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
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use crate::db::{open_database, run_migrations};
    use tempfile::TempDir;

    fn setup_db() -> (TempDir, crate::db::Database) {
        let dir = TempDir::new().expect("tempdir");
        let db = open_database(dir.path().join("test.db")).expect("open");
        run_migrations(&db).expect("migrate");
        db.with_connection(|c| {
            c.execute(
                "INSERT INTO providers (id, name, kind, base_url, encrypted_api_key) \
                 VALUES ('p1', 'OpenAI', 'openai', 'https://api', x'00')",
                [],
            )?;
            c.execute(
                "INSERT INTO providers (id, name, kind, base_url, encrypted_api_key) \
                 VALUES ('p2', 'Claude', 'anthropic', 'https://api', x'00')",
                [],
            )?;
            Ok(())
        })
        .expect("seed");
        (dir, db)
    }

    #[test]
    fn timeseries_returns_buckets() {
        let (_dir, db) = setup_db();
        db.with_connection(|c| {
            for i in 0..5 {
                c.execute(
                    "INSERT INTO request_logs \
                     (timestamp, provider_id, model, status, latency_ms, stream) \
                     VALUES ('2026-04-29T12:00:00.000Z', 'p1', 'gpt-4', ?1, ?2, 0)",
                    rusqlite::params![if i < 4 { "ok" } else { "error" }, 100 + i * 10],
                )?;
            }
            Ok(())
        })
        .expect("insert logs");

        let buckets: Vec<super::MetricsBucket> = db
            .with_connection(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT \
                       strftime('%Y-%m-%dT%H:', timestamp) || \
                         printf('%02d', (CAST(strftime('%M', timestamp) AS INTEGER) / 5) * 5) \
                         || ':00Z' AS timestamp, \
                       COUNT(*), \
                       SUM(CASE WHEN status='ok' THEN 1 ELSE 0 END), \
                       ROUND(1.0 * SUM(CASE WHEN status='ok' THEN 1 ELSE 0 END) / COUNT(*), 4), \
                       ROUND(AVG(latency_ms), 1), \
                       MAX(latency_ms) \
                     FROM request_logs GROUP BY timestamp",
                )?;
                let rows = stmt.query_map([], |row| {
                    Ok(super::MetricsBucket {
                        timestamp: row.get(0)?,
                        request_count: row.get(1)?,
                        success_count: row.get(2)?,
                        success_rate: row.get(3)?,
                        avg_latency_ms: row.get(4)?,
                        p95_latency_ms: row.get(5)?,
                    })
                })?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .expect("query");
        assert!(!buckets.is_empty());
        assert_eq!(buckets[0].request_count, 5);
        assert_eq!(buckets[0].success_count, 4);
        assert!((buckets[0].success_rate - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn cost_summary_groups_by_provider() {
        let (_dir, db) = setup_db();
        db.with_connection(|c| {
            c.execute(
                "UPDATE providers SET cost_per_1k_tokens=0.02 WHERE id='p1'",
                [],
            )?;
            c.execute(
                "UPDATE providers SET cost_per_1k_tokens=0.03 WHERE id='p2'",
                [],
            )?;
            Ok(())
        })
        .expect("set costs");
        db.with_connection(|c| {
            c.execute(
                "INSERT INTO request_logs \
                 (provider_id, model, status, latency_ms, total_tokens, stream) \
                 VALUES ('p1', 'gpt-4', 'ok', 100, 500, 0)",
                [],
            )?;
            c.execute(
                "INSERT INTO request_logs \
                 (provider_id, model, status, latency_ms, total_tokens, stream) \
                 VALUES ('p2', 'claude', 'ok', 80, 300, 0)",
                [],
            )?;
            Ok(())
        })
        .expect("insert");

        let summaries: Vec<super::ProviderCostSummary> = db
            .with_connection(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT p.id, p.name, COALESCE(SUM(total_tokens), 0), \
                            ROUND(COALESCE(SUM( \
                              COALESCE(total_tokens, 0) * COALESCE(p.cost_per_1k_tokens, 0.0) / 1000.0 \
                            ), 0.0), 6), COUNT(request_logs.id) \
                     FROM request_logs \
                     JOIN providers p ON p.id = request_logs.provider_id \
                     GROUP BY p.id, p.name \
                     ORDER BY 4 DESC, 5 DESC",
                )?;
                let rows = stmt.query_map([], |row| {
                    Ok(super::ProviderCostSummary {
                        provider_id: row.get(0)?,
                        provider_name: row.get(1)?,
                        total_tokens: row.get(2)?,
                        total_cost: row.get(3)?,
                        request_count: row.get(4)?,
                    })
                })?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .expect("query");
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].provider_id, "p1");
        assert_eq!(summaries[0].provider_name, "OpenAI");
        assert!((summaries[0].total_cost - 0.01).abs() < f64::EPSILON);
    }

    #[test]
    fn switch_timeline_returns_events() {
        let (_dir, db) = setup_db();
        db.with_connection(|c| {
            c.execute(
                "INSERT INTO switch_history \
                 (from_provider, to_provider, strategy, reason, attempts) \
                 VALUES ('p1', 'p2', 'priority', 'upstream_500', 2)",
                [],
            )
        })
        .expect("insert");

        let events: Vec<super::SwitchEvent> = db
            .with_connection(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, timestamp, from_provider, to_provider, \
                            strategy, reason, attempts \
                     FROM switch_history ORDER BY timestamp DESC LIMIT 50",
                )?;
                let rows = stmt.query_map([], |row| {
                    Ok(super::SwitchEvent {
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
            .expect("query");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].to_provider, "p2");
    }

    #[test]
    fn switch_timeline_marks_deleted_to_provider() {
        let (_dir, db) = setup_db();
        db.with_connection(|c| {
            c.execute(
                "INSERT INTO switch_history \
                 (from_provider, to_provider, strategy, reason, attempts) \
                 VALUES ('p1', 'p2', 'priority', 'upstream_500', 2)",
                [],
            )?;
            c.execute("DELETE FROM providers WHERE id='p2'", [])?;
            Ok(())
        })
        .expect("insert and delete");

        let events = super::query_switch_timeline(&db).expect("timeline");

        assert_eq!(events[0].to_provider, super::DELETED_PROVIDER_LABEL);
    }

    #[test]
    fn provider_quota_columns_exist_after_migration() {
        let (_dir, db) = setup_db();
        db.with_connection(|c| {
            c.execute(
                "UPDATE providers SET monthly_quota=1000, rate_limit_rpm=60, \
                 cost_per_1k_tokens=0.03 WHERE id='p1'",
                [],
            )
        })
        .expect("quota columns must be writable after migration 4");
    }
}
