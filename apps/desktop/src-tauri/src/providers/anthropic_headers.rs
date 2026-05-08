use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::RequestBuilder;

pub(crate) const ANTHROPIC_VERSION: &str = "2023-06-01";
pub(crate) const ANTHROPIC_BETA: &str = "claude-code-20250219,interleaved-thinking-2025-05-14";
pub(crate) const CLAUDE_CLI_USER_AGENT: &str = "claude-cli/2.1.2 (external, cli)";

const CLAUDE_COMPATIBLE_HEADERS: &[(&str, &str)] = &[
    ("anthropic-version", ANTHROPIC_VERSION),
    ("anthropic-beta", ANTHROPIC_BETA),
    ("anthropic-dangerous-direct-browser-access", "true"),
    ("content-type", "application/json"),
    ("accept", "application/json"),
    ("accept-encoding", "identity"),
    ("accept-language", "*"),
    ("user-agent", CLAUDE_CLI_USER_AGENT),
    ("x-app", "cli"),
    ("x-stainless-lang", "js"),
    ("x-stainless-package-version", "0.70.0"),
    ("x-stainless-runtime", "node"),
    ("x-stainless-runtime-version", "v22.20.0"),
    ("x-stainless-retry-count", "0"),
    ("x-stainless-timeout", "600"),
    ("sec-fetch-mode", "cors"),
];

pub(crate) fn insert_claude_compatible_headers(headers: &mut HeaderMap) {
    for &(name, value) in CLAUDE_COMPATIBLE_HEADERS {
        headers.insert(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
    headers.insert(
        HeaderName::from_static("x-stainless-os"),
        HeaderValue::from_static(os_name()),
    );
    headers.insert(
        HeaderName::from_static("x-stainless-arch"),
        HeaderValue::from_static(arch_name()),
    );
}

pub(crate) fn apply_claude_compatible_headers(mut builder: RequestBuilder) -> RequestBuilder {
    for &(name, value) in CLAUDE_COMPATIBLE_HEADERS {
        builder = builder.header(name, value);
    }
    builder
        .header("x-stainless-os", os_name())
        .header("x-stainless-arch", arch_name())
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
