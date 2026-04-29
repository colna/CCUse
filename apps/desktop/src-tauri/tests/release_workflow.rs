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
fn release_workflow_preserves_official_release_and_artifact_contract() {
    for needle in [
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
