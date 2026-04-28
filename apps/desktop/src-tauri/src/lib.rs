//! `CCUse` desktop runtime entry.
//!
//! `main.rs` delegates to [`run`] so the same entry point can be reused
//! by the future mobile target. The local proxy server, providers, and
//! switch engine will be wired in here in later phases.

pub mod proxy;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Err(err) = tauri::Builder::default().run(tauri::generate_context!()) {
        // Surface the cause to stderr; exit non-zero so the OS / launcher
        // can detect that startup failed instead of swallowing the error.
        eprintln!("CCUse failed to start: {err}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    /// Smoke test: keeps the lib crate test target alive so future
    /// modules can drop in `#[test]` items without scaffolding noise.
    #[test]
    fn lib_crate_smoke_test_runs() {
        assert_eq!(2 + 2, 4);
    }
}
