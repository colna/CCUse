//! Integration tests for `tauri.conf.json`.
//!
//! These pin the product-level invariants that `CCUse` depends on:
//! the 3-artifact release contract, brand identity, and platform
//! baselines. Drifting any of these would silently change what gets
//! shipped, so they are guarded by tests rather than review-only.

use serde_json::Value;
use std::fs;

fn load_conf() -> Value {
    let content = fs::read_to_string("tauri.conf.json")
        .expect("tauri.conf.json should be readable from crate root");
    serde_json::from_str(&content).expect("tauri.conf.json should be valid JSON")
}

fn load_cargo_toml() -> String {
    fs::read_to_string("Cargo.toml").expect("Cargo.toml should be readable from crate root")
}

#[test]
fn product_identity_matches_brand() {
    let conf = load_conf();
    assert_eq!(conf["productName"], "CCUse");
    assert_eq!(conf["identifier"], "io.ccuse.desktop");
}

#[test]
fn bundle_targets_locked_to_dmg_and_nsis_only() {
    let conf = load_conf();
    let targets = conf["bundle"]["targets"]
        .as_array()
        .expect("bundle.targets must be an array");
    let names: Vec<&str> = targets.iter().filter_map(Value::as_str).collect();
    // Hard contract: 3 release artifacts (mac aarch64.dmg + mac x64.dmg + win x64-setup.exe).
    // No universal dmg, no .msi, no .app, no .deb.
    assert_eq!(
        names.len(),
        2,
        "bundle.targets must contain exactly [dmg, nsis], found {names:?}"
    );
    assert!(names.contains(&"dmg"), "missing dmg target");
    assert!(names.contains(&"nsis"), "missing nsis target");
}

#[test]
fn macos_minimum_version_is_big_sur() {
    let conf = load_conf();
    assert_eq!(
        conf["bundle"]["macOS"]["minimumSystemVersion"], "11.0",
        "macOS baseline locked to Big Sur per docs §5.3"
    );
}

#[test]
fn windows_uses_downloaded_webview2_bootstrapper() {
    let conf = load_conf();
    assert_eq!(
        conf["bundle"]["windows"]["webviewInstallMode"]["type"], "downloadBootstrapper",
        "Win10 1809+ may lack WebView2 — installer must bootstrap it"
    );
}

#[test]
fn updater_artifacts_disabled_until_updater_phase() {
    let conf = load_conf();
    // Tauri updater is wired in 1.2; until then we don't emit .sig sidecars.
    assert_eq!(conf["bundle"]["createUpdaterArtifacts"], false);
}

#[test]
fn dev_url_points_at_local_vite_port() {
    let conf = load_conf();
    assert_eq!(conf["build"]["devUrl"], "http://localhost:5173");
}

#[test]
fn csp_is_set_and_does_not_allow_unsafe_eval() {
    let conf = load_conf();
    let csp = conf["app"]["security"]["csp"]
        .as_str()
        .expect("app.security.csp must be a non-null string");

    // Must not contain unsafe-eval — XSS vector.
    assert!(
        !csp.contains("unsafe-eval"),
        "CSP must not contain 'unsafe-eval', found: {csp}"
    );

    // Must restrict default-src to 'self'.
    assert!(
        csp.contains("default-src 'self'"),
        "CSP must set default-src 'self', found: {csp}"
    );

    // Must allow connections to the local proxy and known API endpoints.
    assert!(
        csp.contains("connect-src"),
        "CSP must define connect-src, found: {csp}"
    );
    for endpoint in [
        "http://127.0.0.1:*",
        "https://api.openai.com",
        "https://api.anthropic.com",
        "https://generativelanguage.googleapis.com",
    ] {
        assert!(
            csp.contains(endpoint),
            "CSP connect-src must include {endpoint}, found: {csp}"
        );
    }

    // Inline styles needed for Tailwind.
    assert!(
        csp.contains("style-src 'self' 'unsafe-inline'"),
        "CSP must allow inline styles for Tailwind, found: {csp}"
    );
}

#[test]
fn version_field_is_present_and_semver_shaped() {
    let conf = load_conf();
    let version = conf["version"].as_str().expect("version must be a string");
    assert_eq!(version, "1.0.1", "release version must be 1.0.1");
    let parts: Vec<&str> = version.split('.').collect();
    assert_eq!(
        parts.len(),
        3,
        "version must be MAJOR.MINOR.PATCH, got {version}"
    );
    for part in parts {
        assert!(
            part.chars().all(|c| c.is_ascii_digit()),
            "version segment {part} should be numeric"
        );
    }
}

#[test]
fn cargo_package_version_matches_tauri_release_version() {
    let conf = load_conf();
    let version = conf["version"].as_str().expect("version must be a string");
    let cargo_toml = load_cargo_toml();
    let expected = format!("version = \"{version}\"");
    assert!(
        cargo_toml.lines().any(|line| line == expected),
        "Cargo.toml package version must match tauri.conf.json version {version}"
    );
}
