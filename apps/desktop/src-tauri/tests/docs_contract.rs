//! Documentation contract tests for user-facing compatibility matrices.

const USER_MANUAL: &str = include_str!("../../../../docs/user-manual.md");
const README: &str = include_str!("../../../../README.md");
const TASK_REPORT: &str = include_str!("../../../../docs/任务报告.md");
const CHANGELOG: &str = include_str!("../../../../CHANGELOG.md");
const SMOKE_REPORT: &str = include_str!("../../../../docs/smoke-test-report.md");
const TASK_REVIEW: &str = include_str!("../../../../docs/1.0.x-review.md");

fn assert_doc_contains(doc: &str, label: &str, needle: &str) {
    assert!(doc.contains(needle), "{label} must contain `{needle}`");
}

fn assert_manual_contains(needle: &str) {
    assert_doc_contains(USER_MANUAL, "user manual", needle);
}

fn assert_readme_contains(needle: &str) {
    assert_doc_contains(README, "README", needle);
}

fn assert_task_report_contains(needle: &str) {
    assert_doc_contains(TASK_REPORT, "task report", needle);
}

fn assert_changelog_contains(needle: &str) {
    assert_doc_contains(CHANGELOG, "CHANGELOG", needle);
}

fn assert_smoke_report_contains(needle: &str) {
    assert_doc_contains(SMOKE_REPORT, "smoke test report", needle);
}

fn assert_task_review_contains(needle: &str) {
    assert_doc_contains(TASK_REVIEW, "1.0.x task review", needle);
}

#[test]
fn user_manual_documents_supported_local_api_endpoint_matrix() {
    for needle in [
        "## Supported Local API Endpoints",
        "`GET /v1/models`",
        "`POST /v1/chat/completions`",
        "`POST /v1/messages`",
    ] {
        assert_manual_contains(needle);
    }
}

#[test]
fn user_manual_documents_fields_streaming_and_tool_calling_in_both_languages() {
    for needle in [
        "Request fields accepted",
        "Response fields returned",
        "Streaming",
        "Tool Calling",
        "已接收请求字段",
        "返回字段",
        "流式",
        "工具调用",
        "`data: [DONE]`",
        "`message_start`",
        "`input_json_delta`",
    ] {
        assert_manual_contains(needle);
    }
}

#[test]
fn user_manual_documents_auth_and_unsupported_local_api_surfaces() {
    for needle in [
        "`Authorization: Bearer sk-local-...`",
        "`x-api-key: sk-local-...`",
        "`/v1/responses`",
        "embeddings",
        "fine-tuning",
    ] {
        assert_manual_contains(needle);
    }
}

#[test]
fn readme_documents_chat_completions_curl_quick_check() {
    for needle in [
        "## 快速开始",
        "### 本地 API 快速验证",
        "curl -sS http://127.0.0.1:8787/v1/chat/completions",
        "-H \"Authorization: Bearer sk-local-...\"",
        "\"model\": \"gpt-5.4\"",
        "\"stream\": false",
        "providers_not_configured",
    ] {
        assert_readme_contains(needle);
    }
}

#[test]
fn task_report_documents_deferred_proxy_wiring_revision() {
    for needle in [
        "## 修订 [2026-04-29 21:45] T1.0.6.24 任务报告补登修订",
        "`T1.0.2.15`",
        "`T1.0.3.04`",
        "`T1.0.6.05`–`T1.0.6.10`",
        "不回写、不删除、不改写既有历史记录",
    ] {
        assert_task_report_contains(needle);
    }
}

#[test]
fn changelog_documents_1_0_1_proxy_endpoint_fix() {
    for needle in [
        "## [1.0.1] - 2026-04-29",
        "`fix(proxy): wire /v1/* HTTP routes to SwitchEngine`",
        "`GET /v1/models`",
        "`POST /v1/chat/completions`",
        "`POST /v1/messages`",
        "Anthropic SSE events",
        "README `curl` quick check",
    ] {
        assert_changelog_contains(needle);
    }
}

#[test]
fn smoke_report_documents_release_assets_and_clean_install_scope() {
    for needle in [
        "# CCUse v1.0.1 Smoke Test Report",
        "https://github.com/colna/CCUse/releases/tag/v1.0.1",
        "`CCUse_1.0.1_aarch64.dmg`",
        "`CCUse_1.0.1_x64.dmg`",
        "`CCUse_1.0.1_x64-setup.exe`",
        "macOS Apple Silicon",
        "macOS Intel",
        "Windows x64",
        "Manual clean VM installer smoke",
        "Run `25146771380`",
        "Run `25146771404`",
        "Not executed against the NSIS installer",
    ] {
        assert_smoke_report_contains(needle);
    }
}

#[test]
fn task_review_documents_completion_scope_and_remaining_exceptions() {
    for needle in [
        "# CCUse 1.0.x Task Completion Review",
        "146 planned desktop tasks",
        "`T1.0.1.01` through `T1.0.1.27`",
        "`T1.0.2.04-08`",
        "`T1.0.3.01-11`",
        "`T1.0.4.01-25`",
        "`T1.0.5.18`, `T1.0.5.19`",
        "`T1.0.5.20`",
        "`T1.0.5.21`, `T1.0.6.33`",
        "`T1.0.6.33`",
        "v1.0.0",
        "v1.0.1",
        "Core Functionality Re-Verification",
        "Generated URL and key",
        "Multi-provider failover",
        "chat_completions_retries_after_429_and_uses_next_provider",
        "Authorization: Bearer ...",
        "x-api-key",
        "not fully complete as written",
    ] {
        assert_task_review_contains(needle);
    }
}
