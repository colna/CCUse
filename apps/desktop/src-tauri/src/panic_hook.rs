//! Global panic hook that persists crash context to disk before the
//! process aborts.
//!
//! The crash file lands in the app data directory so support can ask
//! users to attach it. Each crash appends a fenced block so multiple
//! panics (e.g. in test binaries) don't overwrite each other.

use std::io::Write;
use std::path::{Path, PathBuf};

/// File name within the app data directory.
pub const CRASH_FILE_NAME: &str = "crash.log";

/// Install the panic hook. Call once during startup, **before** any
/// `tokio::spawn` or `std::thread::spawn`.
///
/// `app_data_dir` is the directory where the crash log will be
/// written. The hook creates it if missing.
pub fn install_panic_hook(app_data_dir: PathBuf) {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let crash_path = app_data_dir.join(CRASH_FILE_NAME);
        let _ = write_crash_entry(&crash_path, info);
        eprintln!(
            "CCUse: panic captured — crash log written to {}",
            crash_path.display()
        );
        prev(info);
    }));
}

/// Format and append one crash entry. Best-effort: returns any IO
/// error rather than panicking (re-panicking inside a panic hook
/// causes an immediate abort with no useful output).
#[allow(clippy::incompatible_msrv)]
fn write_crash_entry(path: &Path, info: &std::panic::PanicHookInfo<'_>) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;

    let timestamp = format_utc_now();

    let message = if let Some(s) = info.payload().downcast_ref::<&str>() {
        (*s).to_owned()
    } else if let Some(s) = info.payload().downcast_ref::<String>() {
        s.clone()
    } else {
        "(non-string panic payload)".to_owned()
    };

    let location = info
        .location()
        .map_or_else(|| "<unknown>".to_owned(), |loc| format!("{loc}"));

    let backtrace = std::backtrace::Backtrace::force_capture();

    writeln!(file, "--- CRASH at {timestamp} ---")?;
    writeln!(file, "message:  {message}")?;
    writeln!(file, "location: {location}")?;
    writeln!(file, "backtrace:\n{backtrace}")?;
    writeln!(file, "--- END ---\n")?;

    Ok(())
}

/// Format current time as an approximate UTC ISO 8601 string without
/// pulling in `chrono` or `humantime`. Good enough for crash logs.
#[allow(clippy::many_single_char_names)]
fn format_utc_now() -> String {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = dur.as_secs();
    let secs_in_day: u64 = 86400;
    let days = total_secs / secs_in_day;
    let day_secs = total_secs % secs_in_day;
    let h = day_secs / 3600;
    let m = (day_secs % 3600) / 60;
    let s = day_secs % 60;

    let (y, mo, d) = civil_from_days(i64::try_from(days).unwrap_or(0));
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

/// Convert days since 1970-01-01 to (year, month, day) via Howard
/// Hinnant's public-domain civil-from-days algorithm.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn civil_from_days(mut z: i64) -> (i64, u32, u32) {
    z += 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_utc_now_looks_like_iso8601() {
        let ts = format_utc_now();
        assert_eq!(ts.len(), 20);
        assert!(ts.ends_with('Z'));
        assert_eq!(&ts[4..5], "-");
        assert_eq!(&ts[7..8], "-");
        assert_eq!(&ts[10..11], "T");
    }

    #[test]
    fn civil_from_days_epoch_is_1970_01_01() {
        let (y, m, d) = civil_from_days(0);
        assert_eq!((y, m, d), (1970, 1, 1));
    }

    #[test]
    fn civil_from_days_known_date() {
        // 2026-04-28 is day 20_571 since epoch.
        let (y, m, d) = civil_from_days(20_571);
        assert_eq!((y, m, d), (2026, 4, 28));
    }

    #[test]
    fn crash_file_name_is_stable() {
        assert_eq!(CRASH_FILE_NAME, "crash.log");
    }

    #[test]
    fn install_panic_hook_writes_crash_log_on_panic() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let app_dir = dir.path().to_path_buf();
        let crash_path = app_dir.join(CRASH_FILE_NAME);

        install_panic_hook(app_dir);

        // Trigger a panic in a catch_unwind — the hook fires, writes
        // the crash file, then the original default hook prints to
        // stderr, then catch_unwind recovers.
        let _ = std::panic::catch_unwind(|| {
            panic!("deliberate test panic");
        });

        assert!(crash_path.exists(), "crash log must be created");
        let contents = std::fs::read_to_string(&crash_path).expect("read");
        assert!(
            contents.contains("deliberate test panic"),
            "crash log must contain the panic message, got: {contents}",
        );
        assert!(contents.contains("CRASH at"));
        assert!(contents.contains("location:"));
        assert!(contents.contains("--- END ---"));

        // Restore default hook to avoid polluting other tests.
        let _ = std::panic::take_hook();
    }
}
