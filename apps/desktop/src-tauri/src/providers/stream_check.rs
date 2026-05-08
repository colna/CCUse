//! Stream-based provider health checks.
//!
//! Mirrors cc-switch's model-test semantics: send a real streaming
//! request, read only the first chunk, and classify the provider from
//! that result instead of relying on `/v1/models`.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::api::HealthStatus;
use super::error_format::format_reqwest_error;
use super::model::{Provider, ProviderKind};

pub const DEFAULT_STREAM_CHECK_TIMEOUT_SECS: u64 = 45;
pub const DEFAULT_STREAM_CHECK_MAX_RETRIES: u32 = 2;
pub const DEFAULT_DEGRADED_THRESHOLD_MS: u64 = 6000;
pub const DEFAULT_CLAUDE_TEST_MODEL: &str = "claude-haiku-4-5-20251001";
pub const DEFAULT_CODEX_TEST_MODEL: &str = "gpt-5.4@low";
pub const DEFAULT_GEMINI_TEST_MODEL: &str = "gemini-3-flash-preview";
pub const DEFAULT_TEST_PROMPT: &str = "Who are you?";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamCheckConfig {
    pub timeout_secs: u64,
    pub max_retries: u32,
    pub degraded_threshold_ms: u64,
    pub claude_model: String,
    pub codex_model: String,
    pub gemini_model: String,
    pub test_prompt: String,
}

impl Default for StreamCheckConfig {
    fn default() -> Self {
        Self {
            timeout_secs: DEFAULT_STREAM_CHECK_TIMEOUT_SECS,
            max_retries: DEFAULT_STREAM_CHECK_MAX_RETRIES,
            degraded_threshold_ms: DEFAULT_DEGRADED_THRESHOLD_MS,
            claude_model: DEFAULT_CLAUDE_TEST_MODEL.to_owned(),
            codex_model: DEFAULT_CODEX_TEST_MODEL.to_owned(),
            gemini_model: DEFAULT_GEMINI_TEST_MODEL.to_owned(),
            test_prompt: DEFAULT_TEST_PROMPT.to_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StreamCheckStatus {
    Operational,
    Degraded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamCheckResult {
    pub status: StreamCheckStatus,
    pub success: bool,
    pub message: String,
    pub response_time_ms: Option<u64>,
    pub http_status: Option<u16>,
    pub model_used: String,
    pub tested_at: i64,
    pub retry_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_category: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum StreamCheckError {
    #[error("failed to create stream check client: {0}")]
    Client(String),
}

#[derive(Debug, Clone)]
pub struct StreamCheckService {
    client: Client,
    config: StreamCheckConfig,
}

impl StreamCheckService {
    pub fn new(config: StreamCheckConfig) -> Result<Self, StreamCheckError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(|err| StreamCheckError::Client(format_reqwest_error(err)))?;
        Ok(Self { client, config })
    }

    pub fn with_default_config() -> Result<Self, StreamCheckError> {
        Self::new(StreamCheckConfig::default())
    }

    pub fn config(&self) -> &StreamCheckConfig {
        &self.config
    }

    pub async fn check_with_retry(&self, provider: &Provider, api_key: &str) -> StreamCheckResult {
        let mut last_result = None;

        for attempt in 0..=self.config.max_retries {
            let result = self.check_once(provider, api_key).await;
            if result.success {
                return StreamCheckResult {
                    retry_count: attempt,
                    ..result
                };
            }

            if should_retry(&result.message) && attempt < self.config.max_retries {
                last_result = Some(result);
                continue;
            }

            return StreamCheckResult {
                retry_count: attempt,
                ..result
            };
        }

        last_result.unwrap_or_else(|| StreamCheckResult {
            status: StreamCheckStatus::Failed,
            success: false,
            message: "Check failed".to_owned(),
            response_time_ms: None,
            http_status: None,
            model_used: model_for_provider_kind(provider.kind, &self.config).to_owned(),
            tested_at: unix_timestamp_now(),
            retry_count: self.config.max_retries,
            error_category: None,
        })
    }

    async fn check_once(&self, provider: &Provider, api_key: &str) -> StreamCheckResult {
        let start = Instant::now();
        let model = model_for_provider_kind(provider.kind, &self.config);
        let timeout = Duration::from_secs(self.config.timeout_secs);
        let result = match provider.kind {
            ProviderKind::Anthropic => {
                check_anthropic_stream(
                    &self.client,
                    &provider.base_url,
                    api_key,
                    model,
                    &self.config.test_prompt,
                    timeout,
                )
                .await
            }
            ProviderKind::Gemini => {
                check_gemini_stream(
                    &self.client,
                    &provider.base_url,
                    api_key,
                    model,
                    &self.config.test_prompt,
                    timeout,
                )
                .await
            }
            ProviderKind::Openai | ProviderKind::Custom | ProviderKind::Relay => {
                check_openai_chat_stream(
                    &self.client,
                    &provider.base_url,
                    api_key,
                    model,
                    &self.config.test_prompt,
                    timeout,
                )
                .await
            }
        };

        build_result(result, start, self.config.degraded_threshold_ms, model)
    }
}

pub async fn check_provider_with_default_config(
    provider: &Provider,
    api_key: &str,
) -> Result<StreamCheckResult, StreamCheckError> {
    let service = StreamCheckService::with_default_config()?;
    Ok(service.check_with_retry(provider, api_key).await)
}

fn model_for_provider_kind(kind: ProviderKind, config: &StreamCheckConfig) -> &str {
    match kind {
        ProviderKind::Anthropic => &config.claude_model,
        ProviderKind::Gemini => &config.gemini_model,
        ProviderKind::Openai | ProviderKind::Custom | ProviderKind::Relay => &config.codex_model,
    }
}

async fn check_anthropic_stream(
    client: &Client,
    base_url: &str,
    api_key: &str,
    model: &str,
    test_prompt: &str,
    timeout: Duration,
) -> Result<(u16, String), CheckFailure> {
    let url = append_endpoint(base_url, "/v1/messages");
    let body = json!({
        "model": model,
        "max_tokens": 1,
        "messages": [{ "role": "user", "content": test_prompt }],
        "stream": true,
    });

    let response = client
        .post(url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header(
            "anthropic-beta",
            "claude-code-20250219,interleaved-thinking-2025-05-14",
        )
        .header("anthropic-dangerous-direct-browser-access", "true")
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .header("accept-encoding", "identity")
        .header("accept-language", "*")
        .header("user-agent", "claude-cli/2.1.2 (external, cli)")
        .header("x-app", "cli")
        .header("x-stainless-lang", "js")
        .header("x-stainless-package-version", "0.70.0")
        .header("x-stainless-os", os_name())
        .header("x-stainless-arch", arch_name())
        .header("x-stainless-runtime", "node")
        .header("x-stainless-runtime-version", "v22.20.0")
        .header("x-stainless-retry-count", "0")
        .header("x-stainless-timeout", "600")
        .header("sec-fetch-mode", "cors")
        .timeout(timeout)
        .json(&body)
        .send()
        .await
        .map_err(map_request_error)?;

    read_first_chunk(response, model).await
}

async fn check_openai_chat_stream(
    client: &Client,
    base_url: &str,
    api_key: &str,
    model: &str,
    test_prompt: &str,
    timeout: Duration,
) -> Result<(u16, String), CheckFailure> {
    let url = append_endpoint(base_url, "/v1/chat/completions");
    let (actual_model, _reasoning_effort) = parse_model_with_effort(model);
    let body = openai_chat_body(&actual_model, test_prompt);

    let response = client
        .post(url)
        .header("authorization", format!("Bearer {api_key}"))
        .header("content-type", "application/json")
        .header("accept", "text/event-stream")
        .header("accept-encoding", "identity")
        .timeout(timeout)
        .json(&body)
        .send()
        .await
        .map_err(map_request_error)?;

    read_first_chunk(response, &actual_model).await
}

async fn check_gemini_stream(
    client: &Client,
    base_url: &str,
    api_key: &str,
    model: &str,
    test_prompt: &str,
    timeout: Duration,
) -> Result<(u16, String), CheckFailure> {
    let normalized_model = normalize_gemini_model_id(model);
    let url = gemini_stream_url(base_url, normalized_model);
    let body = json!({
        "contents": [{
            "role": "user",
            "parts": [{ "text": test_prompt }],
        }],
    });

    let response = client
        .post(url)
        .header("x-goog-api-key", api_key)
        .header("content-type", "application/json")
        .header("accept", "text/event-stream")
        .timeout(timeout)
        .json(&body)
        .send()
        .await
        .map_err(map_request_error)?;

    read_first_chunk(response, model).await
}

fn openai_chat_body(model: &str, test_prompt: &str) -> Value {
    json!({
        "model": model,
        "messages": [{ "role": "user", "content": test_prompt }],
        "max_tokens": 1,
        "stream": true,
    })
}

async fn read_first_chunk(
    response: reqwest::Response,
    model_used: &str,
) -> Result<(u16, String), CheckFailure> {
    let status = response.status().as_u16();
    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(CheckFailure::HttpStatus { status, body });
    }

    let mut stream = response.bytes_stream();
    match stream.next().await {
        Some(Ok(_)) => Ok((status, model_used.to_owned())),
        Some(Err(err)) => Err(CheckFailure::Message(format!(
            "Stream read failed: {}",
            format_reqwest_error(err)
        ))),
        None => Err(CheckFailure::Message(
            "No response data received".to_owned(),
        )),
    }
}

fn build_result(
    result: Result<(u16, String), CheckFailure>,
    start: Instant,
    degraded_threshold_ms: u64,
    model_tested: &str,
) -> StreamCheckResult {
    let response_time = elapsed_millis(start);
    let tested_at = unix_timestamp_now();

    match result {
        Ok((status_code, model_used)) => StreamCheckResult {
            status: determine_status(response_time, degraded_threshold_ms),
            success: true,
            message: "Check succeeded".to_owned(),
            response_time_ms: Some(response_time),
            http_status: Some(status_code),
            model_used,
            tested_at,
            retry_count: 0,
            error_category: None,
        },
        Err(CheckFailure::HttpStatus { status, body }) => StreamCheckResult {
            status: StreamCheckStatus::Failed,
            success: false,
            message: classify_http_status(status).to_owned(),
            response_time_ms: Some(response_time),
            http_status: Some(status),
            model_used: model_tested.to_owned(),
            tested_at,
            retry_count: 0,
            error_category: detect_error_category(status, &body).map(str::to_owned),
        },
        Err(CheckFailure::Message(message)) => StreamCheckResult {
            status: StreamCheckStatus::Failed,
            success: false,
            message,
            response_time_ms: Some(response_time),
            http_status: None,
            model_used: model_tested.to_owned(),
            tested_at,
            retry_count: 0,
            error_category: None,
        },
    }
}

fn determine_status(response_time_ms: u64, degraded_threshold_ms: u64) -> StreamCheckStatus {
    if response_time_ms > degraded_threshold_ms {
        StreamCheckStatus::Degraded
    } else {
        StreamCheckStatus::Operational
    }
}

pub(crate) fn health_status_from_stream_result(result: &StreamCheckResult) -> HealthStatus {
    match result.status {
        StreamCheckStatus::Operational => HealthStatus::Healthy,
        StreamCheckStatus::Degraded => HealthStatus::Degraded,
        StreamCheckStatus::Failed => HealthStatus::Down,
    }
}

pub(crate) fn should_retry(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("timeout")
        || lower.contains("timed out")
        || lower.contains("aborted")
        || lower.contains("abort")
}

pub(crate) fn detect_error_category(status: u16, body: &str) -> Option<&'static str> {
    if !(400..500).contains(&status) {
        return None;
    }

    let lower = body.to_lowercase();
    let quota_indicators = [
        "coding_plan_hour_quota_exceeded",
        "coding_plan_week_quota_exceeded",
        "coding_plan_month_quota_exceeded",
    ];
    if quota_indicators
        .iter()
        .any(|indicator| lower.contains(indicator))
    {
        return Some("quotaExceeded");
    }

    if !lower.contains("model") {
        return None;
    }
    let indicators = [
        "model_not_found",
        "model not found",
        "does not exist",
        "invalid_model",
        "invalid model",
        "unknown_model",
        "unknown model",
        "is not a valid model",
        "not_found_error",
    ];
    if indicators.iter().any(|indicator| lower.contains(indicator)) {
        Some("modelNotFound")
    } else {
        None
    }
}

fn classify_http_status(status: u16) -> &'static str {
    match status {
        400 => "Bad request (400)",
        401 => "Auth rejected (401)",
        402 => "Payment required (402)",
        403 => "Access denied (403)",
        404 => "Not found (404)",
        429 => "Rate limited (429)",
        500 => "Internal server error (500)",
        502 => "Bad gateway (502)",
        503 => "Service unavailable (503)",
        504 => "Gateway timeout (504)",
        s if (500..600).contains(&s) => "Server error",
        _ => "HTTP error",
    }
}

fn map_request_error(err: reqwest::Error) -> CheckFailure {
    if err.is_timeout() {
        return CheckFailure::Message("Request timeout".to_owned());
    }

    let prefix = if err.is_connect() {
        "Connection failed"
    } else {
        "Request failed"
    };

    CheckFailure::Message(format!("{prefix}: {}", format_reqwest_error(err)))
}

fn append_endpoint(base_url: &str, endpoint: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let endpoint_without_version = endpoint.trim_start_matches("/v1/");
    if base.ends_with("/v1") {
        format!("{base}/{endpoint_without_version}")
    } else {
        format!("{base}{endpoint}")
    }
}

fn gemini_stream_url(base_url: &str, normalized_model: &str) -> String {
    let base = base_url.trim_end_matches('/');
    if base.contains("/v1beta") || base.contains("/v1/") {
        format!("{base}/models/{normalized_model}:streamGenerateContent?alt=sse")
    } else {
        format!("{base}/v1beta/models/{normalized_model}:streamGenerateContent?alt=sse")
    }
}

fn normalize_gemini_model_id(model: &str) -> &str {
    model.strip_prefix("models/").unwrap_or(model)
}

fn parse_model_with_effort(model: &str) -> (String, Option<String>) {
    if let Some((actual_model, effort)) = model.split_once('@') {
        return (actual_model.to_owned(), Some(effort.to_owned()));
    }
    if let Some((actual_model, effort)) = model.split_once('#') {
        return (actual_model.to_owned(), Some(effort.to_owned()));
    }
    (model.to_owned(), None)
}

fn os_name() -> &'static str {
    match std::env::consts::OS {
        "macos" => "MacOS",
        "linux" => "Linux",
        "windows" => "Windows",
        other => other,
    }
}

fn arch_name() -> &'static str {
    match std::env::consts::ARCH {
        "aarch64" => "arm64",
        "x86_64" => "x86_64",
        "x86" => "x86",
        other => other,
    }
}

fn elapsed_millis(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn unix_timestamp_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}

#[derive(Debug)]
enum CheckFailure {
    HttpStatus { status: u16, body: String },
    Message(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_json, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn provider_with_base(kind: ProviderKind, base_url: String) -> Provider {
        Provider {
            id: "provider-1".to_owned(),
            name: "Provider 1".to_owned(),
            kind,
            base_url,
            priority: 10,
            enabled: true,
            monthly_quota: None,
            rate_limit_rpm: None,
            cost_per_1k_tokens: None,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn default_config_matches_cc_switch_stream_check() {
        let config = StreamCheckConfig::default();

        assert_eq!(config.timeout_secs, 45);
        assert_eq!(config.max_retries, 2);
        assert_eq!(config.degraded_threshold_ms, 6000);
        assert_eq!(config.claude_model, "claude-haiku-4-5-20251001");
        assert_eq!(config.codex_model, "gpt-5.4@low");
        assert_eq!(config.gemini_model, "gemini-3-flash-preview");
        assert_eq!(config.test_prompt, "Who are you?");
    }

    #[test]
    fn url_building_handles_versioned_base_urls() {
        assert_eq!(
            append_endpoint("https://api.openai.com/v1", "/v1/chat/completions"),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            append_endpoint("https://api.anthropic.com", "/v1/messages"),
            "https://api.anthropic.com/v1/messages"
        );
    }

    #[test]
    fn gemini_url_adds_alt_sse_and_normalizes_models_prefix() {
        assert_eq!(
            gemini_stream_url("https://generativelanguage.googleapis.com", "gemini-3-flash-preview"),
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-3-flash-preview:streamGenerateContent?alt=sse"
        );
        assert_eq!(
            normalize_gemini_model_id("models/gemini-2.5-pro"),
            "gemini-2.5-pro"
        );
    }

    #[test]
    fn model_with_reasoning_effort_is_parsed_like_cc_switch() {
        assert_eq!(
            parse_model_with_effort("gpt-5.4@low"),
            ("gpt-5.4".to_owned(), Some("low".to_owned()))
        );
        assert_eq!(
            parse_model_with_effort("o1-preview#high"),
            ("o1-preview".to_owned(), Some("high".to_owned()))
        );
        assert_eq!(
            parse_model_with_effort("gpt-4o-mini"),
            ("gpt-4o-mini".to_owned(), None)
        );
    }

    #[test]
    fn detect_error_category_matches_cc_switch_cases() {
        let openai_404 = r#"{"error":{"message":"The model `gpt-5.1-codex` does not exist or you do not have access to it","code":"model_not_found"}}"#;
        assert_eq!(
            detect_error_category(404, openai_404),
            Some("modelNotFound")
        );

        let anthropic_404 = r#"{"type":"error","error":{"type":"not_found_error","message":"model: claude-deprecated"}}"#;
        assert_eq!(
            detect_error_category(404, anthropic_404),
            Some("modelNotFound")
        );

        let quota = r#"{"error":{"code":"coding_plan_month_quota_exceeded"}}"#;
        assert_eq!(detect_error_category(429, quota), Some("quotaExceeded"));

        assert_eq!(detect_error_category(404, r#"{"error":"Not Found"}"#), None);
        assert_eq!(detect_error_category(500, openai_404), None);
    }

    #[test]
    fn should_retry_matches_transient_failures() {
        assert!(should_retry("Request timeout"));
        assert!(should_retry("request timed out"));
        assert!(should_retry("connection abort"));
        assert!(!should_retry("socket connection was closed unexpectedly"));
        assert!(!should_retry("Bad gateway (502)"));
        assert!(!should_retry("Auth rejected (401)"));
    }

    #[tokio::test]
    async fn openai_stream_check_reads_first_chunk_and_uses_chat_api() {
        let service = StreamCheckService::with_default_config().expect("service");
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header("authorization", "Bearer sk-test"))
            .and(body_json(openai_chat_body("gpt-5.4", DEFAULT_TEST_PROMPT)))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n"),
            )
            .expect(1)
            .mount(&server)
            .await;
        let provider = provider_with_base(ProviderKind::Openai, server.uri());

        let result = service.check_with_retry(&provider, "sk-test").await;

        assert!(result.success);
        assert_eq!(result.http_status, Some(200));
        assert_eq!(result.model_used, "gpt-5.4");
        assert_eq!(result.retry_count, 0);
    }

    #[tokio::test]
    async fn anthropic_stream_check_uses_claude_headers_and_default_model() {
        let service = StreamCheckService::with_default_config().expect("service");
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .and(header("x-api-key", "sk-ant"))
            .and(header("anthropic-version", "2023-06-01"))
            .and(body_json(json!({
                "model": DEFAULT_CLAUDE_TEST_MODEL,
                "max_tokens": 1,
                "messages": [{ "role": "user", "content": DEFAULT_TEST_PROMPT }],
                "stream": true,
            })))
            .respond_with(
                ResponseTemplate::new(200).set_body_string("event: content_block_delta\n\n"),
            )
            .expect(1)
            .mount(&server)
            .await;
        let provider = provider_with_base(ProviderKind::Anthropic, server.uri());

        let result = service.check_with_retry(&provider, "sk-ant").await;

        assert!(result.success);
        assert_eq!(result.model_used, DEFAULT_CLAUDE_TEST_MODEL);
    }

    #[tokio::test]
    async fn gemini_stream_check_uses_native_sse_endpoint() {
        let service = StreamCheckService::with_default_config().expect("service");
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(format!(
                "/v1beta/models/{DEFAULT_GEMINI_TEST_MODEL}:streamGenerateContent"
            )))
            .and(query_param("alt", "sse"))
            .and(header("x-goog-api-key", "sk-gemini"))
            .respond_with(ResponseTemplate::new(200).set_body_string("data: {}\n\n"))
            .expect(1)
            .mount(&server)
            .await;
        let provider = provider_with_base(ProviderKind::Gemini, server.uri());

        let result = service.check_with_retry(&provider, "sk-gemini").await;

        assert!(result.success);
        assert_eq!(result.model_used, DEFAULT_GEMINI_TEST_MODEL);
    }

    #[tokio::test]
    async fn stream_check_marks_transient_server_errors_with_retry_count() {
        let service = StreamCheckService::new(StreamCheckConfig {
            max_retries: 1,
            ..StreamCheckConfig::default()
        })
        .expect("service");
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(502).set_body_string("bad gateway"))
            .expect(1)
            .mount(&server)
            .await;
        let provider = provider_with_base(ProviderKind::Openai, server.uri());

        let result = service.check_with_retry(&provider, "sk-test").await;

        assert!(!result.success);
        assert_eq!(result.retry_count, 0);
        assert_eq!(result.http_status, Some(502));
    }
}
