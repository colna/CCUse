//! T1.0.5.11 — Log sanitization audit.
//!
//! Scans every `.rs` file under `src/` for print / log statements that
//! might leak sensitive data (API keys, Authorization headers, request
//! bodies). The patterns below must never appear inside `eprintln!`,
//! `println!`, `log::*`, or `tracing::*` macro invocations.
//!
//! False-positive allowlist: test modules (`#[cfg(test)]`) are exempt
//! because they never execute in production.

use std::fs;
use std::path::{Path, PathBuf};

/// Collect all `.rs` files under `dir` recursively.
fn collect_rs_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if dir.is_dir() {
        for entry in fs::read_dir(dir).expect("read_dir failed") {
            let entry = entry.expect("dir entry");
            let path = entry.path();
            if path.is_dir() {
                out.extend(collect_rs_files(&path));
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }
    out
}

/// Return `true` if `line` is inside a log/print macro invocation
/// (heuristic: the line itself contains the macro call, or the
/// nearest preceding line with `eprintln!`/`println!`/`log::`/
/// `tracing::` is within 3 lines — covers multi-line format strings).
fn is_log_statement(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.contains("eprintln!")
        || trimmed.contains("println!")
        || trimmed.contains("log::")
        || trimmed.contains("tracing::")
}

/// Sensitive patterns that must never appear in log output.
const SENSITIVE_PATTERNS: &[&str] = &[
    "api_key:",
    "api_key =",
    "{api_key}",
    "{api_key:",
    "\"Authorization\"",
    "\"x-api-key\"",
    "Bearer {",
    "Bearer {}",
    ".api_key",
];

#[test]
fn no_sensitive_data_in_log_statements() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let files = collect_rs_files(&src_dir);
    assert!(
        !files.is_empty(),
        "no .rs files found under src/ — test setup is broken"
    );

    let mut violations = Vec::new();

    for path in &files {
        let content = fs::read_to_string(path).expect("read file");
        let lines: Vec<&str> = content.lines().collect();
        let mut in_test_module = false;

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Track whether we're inside `#[cfg(test)] mod tests`.
            if trimmed.contains("#[cfg(test)]") {
                in_test_module = true;
            }

            // Skip test modules — they may legitimately assert on
            // sensitive-looking strings.
            if in_test_module {
                continue;
            }

            if is_log_statement(trimmed) {
                // Check this line and the next few (multi-line macros).
                let window_end = (i + 4).min(lines.len());
                for (j, window_line) in lines.iter().enumerate().take(window_end).skip(i) {
                    for pattern in SENSITIVE_PATTERNS {
                        if window_line.contains(pattern) {
                            violations.push(format!(
                                "{}:{}: log statement may leak sensitive data (pattern: `{}`)\n    {}",
                                path.display(),
                                j + 1,
                                pattern,
                                window_line.trim(),
                            ));
                        }
                    }
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Log sanitization violations found:\n{}",
        violations.join("\n"),
    );
}

#[test]
fn debug_impls_redact_api_key() {
    // Verify that types with an `api_key` field use `<redacted>` in
    // their Debug impl — not the real field value.
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let files = collect_rs_files(&src_dir);

    let mut violations = Vec::new();

    for path in &files {
        let content = fs::read_to_string(path).expect("read file");
        let lines: Vec<&str> = content.lines().collect();
        let mut in_test_module = false;

        for (i, line) in lines.iter().enumerate() {
            if line.trim().contains("#[cfg(test)]") {
                in_test_module = true;
            }
            if in_test_module {
                continue;
            }

            // Look for `impl ... Debug` blocks that `.field("api_key"` but
            // don't redact.
            if line.contains(".field(\"api_key\"") && !line.contains("<redacted>") {
                violations.push(format!(
                    "{}:{}: Debug impl exposes api_key without redaction\n    {}",
                    path.display(),
                    i + 1,
                    line.trim(),
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Debug redaction violations found:\n{}",
        violations.join("\n"),
    );
}
