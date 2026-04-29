//! Documentation contract tests for user-facing compatibility matrices.

const USER_MANUAL: &str = include_str!("../../../../docs/user-manual.md");
const README: &str = include_str!("../../../../README.md");
const TASK_REPORT: &str = include_str!("../../../../docs/任务报告.md");
const CHANGELOG: &str = include_str!("../../../../CHANGELOG.md");

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
        "\"model\": \"gpt-4o-mini\"",
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
