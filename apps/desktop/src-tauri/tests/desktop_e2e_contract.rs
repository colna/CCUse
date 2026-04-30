//! Contract tests for the desktop Tauri E2E harness.

const DESKTOP_PACKAGE: &str = include_str!("../../package.json");
const TAURI_PLAYWRIGHT_CONFIG: &str = include_str!("../../playwright.tauri.config.ts");
const TAURI_E2E_SPEC: &str = include_str!("../../e2e/tauri-app.spec.ts");
const DESKTOP_E2E_WORKFLOW: &str = include_str!("../../../../.github/workflows/desktop-e2e.yml");

fn assert_contains(doc: &str, label: &str, needle: &str) {
    assert!(doc.contains(needle), "{label} must contain `{needle}`");
}

#[test]
fn package_exposes_tauri_e2e_script() {
    assert_contains(
        DESKTOP_PACKAGE,
        "desktop package",
        "\"e2e:tauri\": \"playwright test --config playwright.tauri.config.ts\"",
    );
}

#[test]
fn playwright_tauri_config_runs_only_real_tauri_spec_serially() {
    for needle in [
        "testDir: \"./e2e\"",
        "testMatch: \"tauri-app.spec.ts\"",
        "workers: 1",
        "fullyParallel: false",
    ] {
        assert_contains(TAURI_PLAYWRIGHT_CONFIG, "Tauri Playwright config", needle);
    }
}

#[test]
fn tauri_spec_uses_tauri_driver_and_covers_proxy_monitor_flow() {
    for needle in [
        "\"tauri:options\"",
        "browserName: \"wry\"",
        "CCUSE_TAURI_APP_PATH",
        "CCUSE_TAURI_E2E",
        "window.location.hash = '#/providers'",
        "/v1/chat/completions",
        "today-requests-card-value",
    ] {
        assert_contains(TAURI_E2E_SPEC, "Tauri E2E spec", needle);
    }
}

#[test]
fn tauri_spec_sends_json_body_for_webdriver_click() {
    for needle in [
        "`/session/${this.sessionId}/element/${element[ELEMENT_KEY]}/click`,",
        "{},",
    ] {
        assert_contains(TAURI_E2E_SPEC, "Tauri E2E spec", needle);
    }
}

#[test]
fn ci_runs_tauri_driver_on_linux_and_windows() {
    for needle in [
        "name: Desktop Tauri E2E",
        "ubuntu-latest",
        "windows-latest",
        "cargo install tauri-driver --locked",
        "webkit2gtk-driver",
        "xvfb-run -a pnpm --filter @ccuse/desktop e2e:tauri",
        "pnpm --filter @ccuse/desktop tauri build --debug --no-bundle --ci",
        "CCUSE_TAURI_E2E: \"1\"",
    ] {
        assert_contains(DESKTOP_E2E_WORKFLOW, "desktop E2E workflow", needle);
    }
}
