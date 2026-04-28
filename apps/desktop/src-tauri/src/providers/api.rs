//! Runtime [`Provider`] trait + request/response shapes.
//!
//! T1.0.1.19 only freezes the contract; the `OpenAI` implementation
//! lands in T1.0.1.20–21 and the `SwitchEngine` (T1.0.2) calls into
//! whichever providers the repository hands it.
//!
//! The trait is intentionally object-safe (`async_trait` + `dyn`)
//! because the `SwitchEngine` keeps a heterogeneous list of providers.

use std::pin::Pin;

use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::Stream;
use serde::{Deserialize, Serialize};

/// What `SwitchEngine` hands to a provider. Wire format is
/// `OpenAI` chat-completions today; format-conversion adapters in
/// T1.0.3 translate Anthropic / Gemini envelopes into this shape
/// before dispatch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiRequest {
    /// Upstream model id (`gpt-4o`, `claude-3-5-sonnet`, ...).
    pub model: String,
    /// Conversation history in `OpenAI` chat-completions form.
    pub messages: Vec<ChatMessage>,
    /// `OpenAI`-style sampling temperature (0..2). `None` ⇒ provider
    /// default (typically 1.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Stop streaming after this many tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// `true` ⇒ caller wants SSE, dispatch goes through
    /// [`Provider::send_stream_request`].
    #[serde(default)]
    pub stream: bool,
}

/// One turn of a chat conversation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatMessage {
    /// `system` / `user` / `assistant` / `tool`.
    pub role: String,
    /// Plain text. Tool/function calls land in T1.0.2+ (richer enum).
    pub content: String,
}

/// Non-streaming response. Full body delivered in one go.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse {
    /// `OpenAI`-shaped response id (e.g. `chatcmpl-abc...`).
    pub id: String,
    /// Model that actually answered (may differ from request when
    /// the upstream rerouted).
    pub model: String,
    /// One per `choices[]`. Phase 1.0.1 always emits exactly one.
    pub choices: Vec<ApiChoice>,
    /// Token accounting from the upstream; absent for some self-
    /// hosted endpoints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<ApiUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiChoice {
    pub index: u32,
    pub message: ChatMessage,
    /// `stop` / `length` / `tool_calls` / `content_filter`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ApiUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Health snapshot exposed by [`Provider::health_check`].
///
/// Distinct from the network ping — providers may report
/// `Degraded` when they're reachable but quota is exhausted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Down,
}

/// Errors a provider may surface while talking to its upstream.
///
/// Keep the variant set small — the `SwitchEngine` decides whether
/// to fail-over by inspecting `is_retriable`, not by pattern-
/// matching on every variant. Carries `String` payloads on purpose:
/// the underlying `reqwest::Error` is bulky and rarely actionable.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    /// Network failure (DNS, TCP, TLS, timeout). Retriable.
    #[error("network error contacting upstream: {0}")]
    Network(String),
    /// HTTP 5xx from upstream. Retriable.
    #[error("upstream returned {status}: {body}")]
    Upstream { status: u16, body: String },
    /// HTTP 401/403. *Not* retriable — fail-over to next provider.
    #[error("upstream rejected the api key: {0}")]
    Unauthorized(String),
    /// HTTP 429. Retriable but with backoff handled by `SwitchEngine`.
    #[error("upstream rate limit hit: {0}")]
    RateLimited(String),
    /// Body decode / shape mismatch.
    #[error("failed to parse upstream response: {0}")]
    Decode(String),
    /// Caller passed a request the provider can't execute (unknown
    /// model, bad parameters). Not retriable.
    #[error("invalid request: {0}")]
    BadRequest(String),
}

impl ProviderError {
    /// Whether the `SwitchEngine` should retry this error against
    /// another provider. Auth / bad-request failures are terminal.
    #[must_use]
    pub const fn is_retriable(&self) -> bool {
        match self {
            Self::Network(_) | Self::Upstream { .. } | Self::RateLimited(_) => true,
            Self::Unauthorized(_) | Self::BadRequest(_) | Self::Decode(_) => false,
        }
    }
}

/// Streaming chunk type. Each chunk is a raw SSE-encoded byte slice
/// (`data: {...}\n\n`); the proxy forwards them verbatim so client
/// SDKs don't need a re-encode pass.
pub type StreamChunk = Result<Bytes, ProviderError>;

/// Boxed stream of chunks. `dyn Stream` keeps the trait object-safe.
pub type StreamingResponse = Pin<Box<dyn Stream<Item = StreamChunk> + Send>>;

/// Provider runtime contract. T1.0.1.19 freezes the shape; the
/// implementations land in T1.0.1.20+.
#[async_trait]
pub trait Provider: Send + Sync + std::fmt::Debug {
    /// Identifier persisted in the `providers` table — used by the
    /// `SwitchEngine` to log which provider answered a request.
    fn id(&self) -> &str;

    /// Display name for the UI / logs.
    fn name(&self) -> &str;

    /// Liveness probe. Implementations should be cheap: prefer
    /// `/v1/models` or a lightweight `GET` over an actual completion.
    async fn health_check(&self) -> Result<HealthStatus, ProviderError>;

    /// Non-streaming dispatch. The full upstream body is parsed and
    /// returned as [`ApiResponse`].
    async fn send_request(&self, request: ApiRequest) -> Result<ApiResponse, ProviderError>;

    /// Streaming dispatch. The returned stream forwards SSE chunks
    /// verbatim. T1.0.1.22 wraps it in `axum::response::Sse`.
    async fn send_stream_request(
        &self,
        request: ApiRequest,
    ) -> Result<StreamingResponse, ProviderError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_error_retriability_matches_phase_1_0_1_policy() {
        assert!(ProviderError::Network("ETIMEDOUT".into()).is_retriable());
        assert!(ProviderError::Upstream {
            status: 502,
            body: String::new(),
        }
        .is_retriable());
        assert!(ProviderError::RateLimited("limit".into()).is_retriable());
        assert!(!ProviderError::Unauthorized("401".into()).is_retriable());
        assert!(!ProviderError::BadRequest("model".into()).is_retriable());
        assert!(!ProviderError::Decode("eof".into()).is_retriable());
    }

    #[test]
    fn api_request_serialises_omitting_default_fields() {
        let req = ApiRequest {
            model: "gpt-4o".into(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: "hi".into(),
            }],
            temperature: None,
            max_tokens: None,
            stream: false,
        };
        let json = serde_json::to_value(&req).expect("serialise");
        // `temperature` / `max_tokens` should be omitted entirely
        // when None — this matches what OpenAI's SDK does.
        assert_eq!(json["model"], "gpt-4o");
        assert!(json.get("temperature").is_none());
        assert!(json.get("max_tokens").is_none());
        assert_eq!(json["stream"], false);
    }

    #[test]
    fn api_response_round_trips_through_json() {
        let resp = ApiResponse {
            id: "chatcmpl-1".into(),
            model: "gpt-4o".into(),
            choices: vec![ApiChoice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".into(),
                    content: "ok".into(),
                },
                finish_reason: Some("stop".into()),
            }],
            usage: Some(ApiUsage {
                prompt_tokens: 5,
                completion_tokens: 1,
                total_tokens: 6,
            }),
        };
        let s = serde_json::to_string(&resp).expect("ser");
        let back: ApiResponse = serde_json::from_str(&s).expect("de");
        assert_eq!(back.id, "chatcmpl-1");
        assert_eq!(back.choices[0].message.content, "ok");
        assert_eq!(back.usage.expect("usage").total_tokens, 6);
    }

    #[test]
    fn health_status_serialises_as_snake_case() {
        let json = serde_json::to_value(HealthStatus::Healthy).expect("ser");
        assert_eq!(json, serde_json::json!("healthy"));
        let json = serde_json::to_value(HealthStatus::Down).expect("ser");
        assert_eq!(json, serde_json::json!("down"));
    }

    /// Object-safety check: if the trait isn't dyn-safe the file
    /// won't compile, which is itself the assertion.
    #[test]
    fn provider_trait_is_object_safe() {
        fn _accepts_dyn(_: &dyn Provider) {}
    }
}
