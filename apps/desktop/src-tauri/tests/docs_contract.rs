//! Documentation contract tests for user-facing compatibility matrices.

const USER_MANUAL: &str = include_str!("../../../../docs/user-manual.md");

fn assert_manual_contains(needle: &str) {
    assert!(
        USER_MANUAL.contains(needle),
        "user manual must contain `{needle}`"
    );
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
