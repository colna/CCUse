//! `SQLite` storage layer for `CCUse`.
//!
//! Phase 1.0.1 initialises the file (WAL mode, `foreign_keys=ON`,
//! 0600 perms on Unix). T1.0.1.15 layers a migration runner; T1.0.1.18
//! adds the `providers` repository on top.

pub mod init;
pub mod migrations;

pub use init::{open_database, Database, DbError, BUSY_TIMEOUT_MS, WAL_CHECKPOINT_INTERVAL};
pub use migrations::run_migrations;
