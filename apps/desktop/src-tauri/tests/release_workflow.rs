//! Contract tests for the GitHub Actions release workflow.

const RELEASE_WORKFLOW: &str = include_str!("../../../../.github/workflows/release.yml");

fn assert_workflow_contains(needle: &str) {
    assert!(
        RELEASE_WORKFLOW.contains(needle),
        "release workflow must contain `{needle}`"
    );
}

fn assert_workflow_excludes(needle: &str) {
    assert!(
        !RELEASE_WORKFLOW.contains(needle),
        "release workflow must not contain stale `{needle}`"
    );
}

#[test]
fn release_workflow_uses_desktop_monorepo_paths() {
    for needle in [
        "apps/desktop/src-tauri/tauri.conf.json",
        "workspaces: apps/desktop/src-tauri",
        "projectPath: apps/desktop",
    ] {
        assert_workflow_contains(needle);
    }

    for needle in [
        "jq -r '.version' src-tauri/tauri.conf.json",
        "version not found in src-tauri/tauri.conf.json",
        "workspaces: src-tauri",
    ] {
        assert_workflow_excludes(needle);
    }
}

#[test]
fn release_workflow_recovers_when_tag_exists_but_release_is_missing() {
    for needle in [
        "gh release view \"$TAG\"",
        "tag_exists: ${{ steps.check.outputs.tag_exists }}",
        "release_exists: ${{ steps.check.outputs.release_exists }}",
        "missing_macos_aarch64: ${{ steps.check.outputs.missing_macos_aarch64 }}",
        "missing_macos_x64: ${{ steps.check.outputs.missing_macos_x64 }}",
        "missing_windows_x64: ${{ steps.check.outputs.missing_windows_x64 }}",
        "needs.check-version.outputs.tag_exists != 'true'",
        "needs.create-tag.result == 'success' || needs.create-tag.result == 'skipped'",
    ] {
        assert_workflow_contains(needle);
    }
}

#[test]
fn release_workflow_only_builds_missing_required_assets() {
    for needle in [
        "CCUse_${VERSION}_aarch64.dmg",
        "CCUse_${VERSION}_x64.dmg",
        "CCUse_${VERSION}_x64-setup.exe",
        "asset_key: macos_aarch64",
        "asset_key: macos_x64",
        "asset_key: windows_x64",
        "matrix.asset_key == 'macos_aarch64'",
        "needs.check-version.outputs.missing_macos_aarch64 == 'true'",
    ] {
        assert_workflow_contains(needle);
    }
}

#[test]
fn release_workflow_preserves_official_release_and_artifact_contract() {
    for needle in [
        "version: 10.30.3",
        "prerelease: ${{ needs.check-version.outputs.is_prerelease == 'true' }}",
        "MAJOR=\"${VERSION%%.*}\"",
        "CCUse_<version>_aarch64.dmg",
        "CCUse_<version>_x64.dmg",
        "CCUse_<version>_x64-setup.exe",
        "`*_aarch64.dmg`",
        "`*_x64.dmg`",
        "`*_x64-setup.exe`",
    ] {
        assert_workflow_contains(needle);
    }
}

#[test]
fn release_workflow_gates_optional_code_signing_secrets() {
    for needle in [
        "vars.APPLE_SIGNING_ENABLED == 'true'",
        "vars.WINDOWS_SIGNING_ENABLED == 'true'",
        "secrets.APPLE_CERTIFICATE || ''",
        "secrets.APPLE_CERTIFICATE_PASSWORD || ''",
        "secrets.APPLE_SIGNING_IDENTITY || ''",
        "secrets.WINDOWS_CERTIFICATE || ''",
    ] {
        assert_workflow_contains(needle);
    }
}
