//! `axum`-based HTTP server scaffolding for the local proxy.
//!
//! Phase 1.0.1 scope: only a `/healthz` endpoint and a clean
//! graceful-shutdown contract. Real routes (`/v1/chat/completions`
//! etc.) are introduced in T1.0.1.08; provider dispatch in T1.0.2.

use std::collections::VecDeque;
use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::State;
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use axum::http::{HeaderName, Method};
use axum::response::{IntoResponse, Json};
use axum::routing::{get, post};
use axum::Router;
use bytes::Bytes;
use futures::StreamExt;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::auth::{require_local_api_key, KeyStore};
use crate::commands::model_mapping::ModelMappingHandle;
use crate::commands::switch::SwitchEngineHandle;
use crate::converter::{
    sse::parse_sse_frames, AnthropicConverter, FormatConverter, ModelMapping, OpenAIConverter,
    UnifiedRequest,
};
use crate::db::Database;
use crate::providers::{ProviderError, ProviderManager, StreamingResponse};
use crate::switch::request_log::{RequestLogInput, RequestLogRepository};

use crate::providers::api::ApiRequest;

use super::error::{ApiError, ApiErrorKind};
use super::{bridge, sse};

/// Errors raised while binding or running the proxy server.
#[derive(thiserror::Error, Debug)]
pub enum ServerError {
    /// Failed to bind a `TcpListener` to the requested address.
    #[error("failed to bind tcp listener at {addr}: {source}")]
    Bind {
        addr: SocketAddr,
        #[source]
        source: std::io::Error,
    },

    /// Probed `attempts` consecutive ports from `start` and none was available.
    #[error(
        "All ports occupied — please free one of ports {start}–{}",
        start.saturating_add(*attempts).saturating_sub(1)
    )]
    NoAvailablePort {
        start: u16,
        attempts: u16,
        #[source]
        last: Option<Box<ServerError>>,
    },

    /// `start + offset` would overflow `u16` while probing.
    #[error("port probe overflowed u16 starting at {start} after {offset} steps")]
    PortOverflow { start: u16, offset: u16 },

    /// `axum::serve` returned an io error while running.
    #[error("axum serve loop exited with error: {0}")]
    Serve(#[from] std::io::Error),
}

/// A bound proxy server, ready to accept connections.
///
/// Two-step lifecycle so callers can read [`ProxyServer::local_addr`]
/// before they hand the server off to a long-lived task. This is what
/// the port-prober (T1.0.1.07) and the tray UI (T1.0.4.15) need to
/// surface "listening on port N" to the user.
pub struct ProxyServer {
    listener: TcpListener,
    local_addr: SocketAddr,
}

#[derive(Debug, Clone)]
pub struct ProxyAppState {
    pub engine: SwitchEngineHandle,
    pub model_mapping: ModelMappingHandle,
    pub manager: Arc<ProviderManager>,
    pub request_log: Option<RequestLogRepository>,
    pub openai_converter: OpenAIConverter,
    pub anthropic_converter: AnthropicConverter,
}

impl ProxyAppState {
    #[must_use]
    pub fn new(
        engine: SwitchEngineHandle,
        model_mapping: Arc<RwLock<ModelMapping>>,
        manager: Arc<ProviderManager>,
    ) -> Self {
        Self {
            engine,
            model_mapping,
            manager,
            request_log: None,
            openai_converter: OpenAIConverter,
            anthropic_converter: AnthropicConverter,
        }
    }

    /// Set the request log repository (requires database).
    #[must_use]
    pub fn with_request_log(mut self, db: Database) -> Self {
        self.request_log = Some(RequestLogRepository::new(db));
        self
    }
}

impl std::fmt::Debug for ProxyServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // listener intentionally omitted — its inner state is not useful
        // in logs and `TcpListener`'s Debug output is verbose.
        f.debug_struct("ProxyServer")
            .field("local_addr", &self.local_addr)
            .finish_non_exhaustive()
    }
}

impl ProxyServer {
    /// Bind to `addr`. Use port `0` to let the OS pick — the actual
    /// port is then available via [`ProxyServer::local_addr`].
    pub async fn bind(addr: SocketAddr) -> Result<Self, ServerError> {
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|source| ServerError::Bind { addr, source })?;
        let local_addr = listener
            .local_addr()
            .map_err(|source| ServerError::Bind { addr, source })?;
        Ok(Self {
            listener,
            local_addr,
        })
    }

    /// Probe ports `[start, start + attempts)` on `127.0.0.1` and bind
    /// to the first available one.
    ///
    /// Drives the desktop default flow: try `8787`, walk up to `8886`
    /// if `8787` is already taken. The bound port is reported via
    /// [`ProxyServer::local_addr`] so the tray / UI can surface it.
    pub async fn bind_with_fallback(start: u16, attempts: u16) -> Result<Self, ServerError> {
        let mut last: Option<ServerError> = None;
        for offset in 0..attempts {
            let port = start
                .checked_add(offset)
                .ok_or(ServerError::PortOverflow { start, offset })?;
            let addr: SocketAddr = ([127, 0, 0, 1], port).into();
            match Self::bind(addr).await {
                Ok(server) => return Ok(server),
                Err(err) => last = Some(err),
            }
        }
        Err(ServerError::NoAvailablePort {
            start,
            attempts,
            last: last.map(Box::new),
        })
    }

    /// The address actually bound. Differs from the `addr` passed to
    /// [`ProxyServer::bind`] when port `0` was requested.
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Run the server until `shutdown` resolves.
    ///
    /// Static dispatch on the shutdown future (per `rust-best-practices`
    /// §6) — callers usually pass `tokio::signal::ctrl_c()` or a
    /// `oneshot::Receiver`; both are zero-cost here.
    pub async fn serve_with_shutdown<F>(
        self,
        state: ProxyAppState,
        shutdown: F,
    ) -> Result<(), ServerError>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let app = build_router(None, state);
        axum::serve(self.listener, app)
            .with_graceful_shutdown(shutdown)
            .await?;
        Ok(())
    }

    /// Run the server with the `sk-local-…` auth middleware mounted
    /// on `/v1/*`. `/healthz` stays open so external probes (tray,
    /// health-check loop) don't need a key.
    pub async fn serve_with_auth_and_shutdown<F>(
        self,
        key_store: KeyStore,
        state: ProxyAppState,
        shutdown: F,
    ) -> Result<(), ServerError>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let app = build_router(Some(key_store), state);
        axum::serve(self.listener, app)
            .with_graceful_shutdown(shutdown)
            .await?;
        Ok(())
    }
}

/// Build the router for the local proxy.
///
/// `/healthz` is the liveness probe used by tray + health-check loop.
/// The three `v1/*` routes are the unified API surface clients call
/// into; their handlers are stubs until T1.0.2 wires the provider
/// dispatch — they return 503 with an `OpenAI`-shaped error body.
///
/// When `auth` is `Some`, the `/v1/*` routes require the
/// `sk-local-…` API key (T1.0.1.13). `/healthz` is always open.
fn build_router(auth: Option<KeyStore>, state: ProxyAppState) -> Router {
    let mut v1 = Router::new()
        .route("/v1/models", get(list_models))
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/messages", post(anthropic_messages))
        .with_state(state);
    if let Some(store) = auth {
        v1 = v1.layer(axum::middleware::from_fn_with_state(
            store,
            require_local_api_key,
        ));
    }
    Router::new()
        .route("/healthz", get(healthz))
        .merge(v1)
        .layer(cors_layer())
}

/// CORS policy.
///
/// Most `CCUse` clients (Cursor / Claude Desktop / Continue) are
/// native apps and never send `Origin`, so they bypass CORS entirely.
/// The policy here exists for the few legitimate browser callers —
/// local dev tooling and the Tauri `WebView` itself — and refuses
/// anything that's not loopback or the Tauri custom scheme.
fn cors_layer() -> CorsLayer {
    let origin = AllowOrigin::predicate(|origin, _request| {
        let raw = origin.to_str().unwrap_or_default();
        raw.starts_with("http://127.0.0.1")
            || raw.starts_with("http://localhost")
            || raw == "tauri://localhost"
    });
    CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([
            AUTHORIZATION,
            CONTENT_TYPE,
            HeaderName::from_static("x-api-key"),
        ])
        .allow_origin(origin)
        .max_age(std::time::Duration::from_secs(600))
}

/// `GET /healthz` — returns `200 ok`. Minimal payload on purpose;
/// the rich health snapshot lives in `T1.0.2.21 get_health_snapshot`
/// (a Tauri command, not an HTTP route).
async fn healthz() -> &'static str {
    "ok"
}

/// `GET /v1/models` — returns an empty list until the provider
/// registry (T1.0.2.03 `ProviderManager`) is wired.
///
/// Shape mirrors `OpenAI`'s `/v1/models` so generic clients see
/// "no models available" instead of a hard 404.
async fn list_models(State(_state): State<ProxyAppState>) -> Json<Value> {
    Json(json!({
        "object": "list",
        "data": [],
    }))
}

/// `POST /v1/chat/completions` — OpenAI-format inbound.
///
/// Flow: parse body → `OpenAIConverter::request_to_unified` → bridge →
/// `SwitchEngine::dispatch` → bridge back → `OpenAIConverter::unified_to_response`.
async fn chat_completions(
    State(state): State<ProxyAppState>,
    body: axum::body::Bytes,
) -> Result<axum::response::Response, ApiError> {
    if state.manager.is_empty().await {
        return Err(ApiError::new(
            ApiErrorKind::NoProvider,
            "No providers configured. Add a provider in CCUse settings.",
        ));
    }

    let body_json: Value =
        serde_json::from_slice(&body).map_err(|e| ApiError::bad_request(e.to_string()))?;

    let unified: UnifiedRequest = state.openai_converter.request_to_unified(&body_json)?;

    let api_req = bridge::unified_to_api_request(&unified);

    if unified.stream {
        return handle_streaming_chat(state, api_req).await;
    }

    let start = std::time::Instant::now();

    let result = state
        .engine
        .dispatch(api_req.clone())
        .await
        .map_err(ApiError::from)?;

    let elapsed = start.elapsed();

    // Fire-and-forget request logging
    if let Some(ref log_repo) = state.request_log {
        let input = RequestLogInput {
            provider_id: result.provider_id.clone(),
            model: api_req.model.clone(),
            status: "ok".into(),
            error_kind: None,
            latency_ms: i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX),
            prompt_tokens: result
                .response
                .usage
                .as_ref()
                .map(|u| i64::from(u.prompt_tokens)),
            completion_tokens: result
                .response
                .usage
                .as_ref()
                .map(|u| i64::from(u.completion_tokens)),
            total_tokens: result
                .response
                .usage
                .as_ref()
                .map(|u| i64::from(u.total_tokens)),
            stream: false,
        };
        let _ = log_repo.insert(&input);
    }

    let unified_resp = bridge::api_response_to_unified(&result.response);
    let out = state.openai_converter.unified_to_response(&unified_resp)?;
    Ok(Json(out).into_response())
}

/// Streaming path for `chat_completions`. All providers currently
/// speak OpenAI-format SSE, so we forward the byte stream verbatim
/// with keep-alive injected. Usage-based logging is skipped because
/// token counts are not available until the stream ends — the
/// non-streaming path handles that.
async fn handle_streaming_chat(
    state: ProxyAppState,
    api_req: ApiRequest,
) -> Result<axum::response::Response, ApiError> {
    let start = std::time::Instant::now();

    let result = state
        .engine
        .dispatch_stream(api_req.clone())
        .await
        .map_err(ApiError::from)?;

    let elapsed = start.elapsed();

    // Log the stream initiation (tokens unknown until stream ends).
    if let Some(ref log_repo) = state.request_log {
        let input = RequestLogInput {
            provider_id: result.provider_id.clone(),
            model: api_req.model.clone(),
            status: "ok".into(),
            error_kind: None,
            latency_ms: i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX),
            prompt_tokens: None,
            completion_tokens: None,
            total_tokens: None,
            stream: true,
        };
        let _ = log_repo.insert(&input);
    }

    let stream = sse::with_keep_alive(result.response, sse::DEFAULT_KEEP_ALIVE);
    Ok(sse::stream_to_sse_response(stream))
}

/// `POST /v1/messages` — Anthropic-format inbound.
///
/// Flow mirrors `chat_completions`, but uses the Anthropic converter
/// at the HTTP boundary so Anthropic SDKs receive `message` responses.
async fn anthropic_messages(
    State(state): State<ProxyAppState>,
    body: axum::body::Bytes,
) -> Result<axum::response::Response, ApiError> {
    if state.manager.is_empty().await {
        return Err(ApiError::new(
            ApiErrorKind::NoProvider,
            "No providers configured. Add a provider in CCUse settings.",
        ));
    }

    let body_json: Value =
        serde_json::from_slice(&body).map_err(|e| ApiError::bad_request(e.to_string()))?;

    let unified: UnifiedRequest = state.anthropic_converter.request_to_unified(&body_json)?;
    let api_req = bridge::unified_to_api_request(&unified);
    if unified.stream {
        return handle_streaming_anthropic_messages(state, api_req).await;
    }

    let start = std::time::Instant::now();
    let result = state
        .engine
        .dispatch(api_req.clone())
        .await
        .map_err(ApiError::from)?;
    let elapsed = start.elapsed();

    if let Some(ref log_repo) = state.request_log {
        let input = RequestLogInput {
            provider_id: result.provider_id.clone(),
            model: api_req.model.clone(),
            status: "ok".into(),
            error_kind: None,
            latency_ms: i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX),
            prompt_tokens: result
                .response
                .usage
                .as_ref()
                .map(|u| i64::from(u.prompt_tokens)),
            completion_tokens: result
                .response
                .usage
                .as_ref()
                .map(|u| i64::from(u.completion_tokens)),
            total_tokens: result
                .response
                .usage
                .as_ref()
                .map(|u| i64::from(u.total_tokens)),
            stream: false,
        };
        let _ = log_repo.insert(&input);
    }

    let unified_resp = bridge::api_response_to_unified(&result.response);
    let out = state
        .anthropic_converter
        .unified_to_response(&unified_resp)?;
    Ok(Json(out).into_response())
}

async fn handle_streaming_anthropic_messages(
    state: ProxyAppState,
    api_req: ApiRequest,
) -> Result<axum::response::Response, ApiError> {
    let start = std::time::Instant::now();
    let result = state
        .engine
        .dispatch_stream(api_req.clone())
        .await
        .map_err(ApiError::from)?;
    let elapsed = start.elapsed();

    if let Some(ref log_repo) = state.request_log {
        let input = RequestLogInput {
            provider_id: result.provider_id.clone(),
            model: api_req.model.clone(),
            status: "ok".into(),
            error_kind: None,
            latency_ms: i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX),
            prompt_tokens: None,
            completion_tokens: None,
            total_tokens: None,
            stream: true,
        };
        let _ = log_repo.insert(&input);
    }

    let stream = openai_sse_to_anthropic_sse(
        result.response,
        state.openai_converter,
        state.anthropic_converter,
    );
    let stream = sse::with_keep_alive(stream, sse::DEFAULT_KEEP_ALIVE);
    Ok(sse::stream_to_sse_response(stream))
}

struct AnthropicSseBridge {
    openai: OpenAIConverter,
    anthropic: AnthropicConverter,
    text_block_started: bool,
    text_block_stopped: bool,
}

impl AnthropicSseBridge {
    fn new(openai: OpenAIConverter, anthropic: AnthropicConverter) -> Self {
        Self {
            openai,
            anthropic,
            text_block_started: false,
            text_block_stopped: false,
        }
    }

    fn push_frame_results(
        &mut self,
        raw: &str,
        pending: &mut VecDeque<Result<Bytes, ProviderError>>,
    ) {
        for frame in parse_sse_frames(raw) {
            if frame.data == "[DONE]" {
                self.push_done(pending);
                continue;
            }

            let chunk = match self.openai.parse_stream_chunk(&frame.data) {
                Ok(Some(chunk)) => chunk,
                Ok(None) => continue,
                Err(err) => {
                    pending.push_back(Err(ProviderError::Decode(err.to_string())));
                    continue;
                }
            };

            let has_text_delta = chunk.choices.iter().any(|choice| {
                choice
                    .delta
                    .as_ref()
                    .and_then(|delta| delta.content.as_ref())
                    .is_some()
            });
            let has_finish = chunk
                .choices
                .iter()
                .any(|choice| choice.finish_reason.is_some());

            if has_text_delta && !self.text_block_started {
                self.text_block_started = true;
                pending.push_back(Ok(content_block_start_frame()));
            }
            if has_finish && self.text_block_started && !self.text_block_stopped {
                self.text_block_stopped = true;
                pending.push_back(Ok(content_block_stop_frame()));
            }

            match self.anthropic.encode_stream_chunk(&chunk) {
                Ok(encoded) if !encoded.is_empty() => pending.push_back(Ok(Bytes::from(encoded))),
                Ok(_) => {}
                Err(err) => pending.push_back(Err(ProviderError::Decode(err.to_string()))),
            }
        }
    }

    fn push_done(&mut self, pending: &mut VecDeque<Result<Bytes, ProviderError>>) {
        if self.text_block_started && !self.text_block_stopped {
            self.text_block_stopped = true;
            pending.push_back(Ok(content_block_stop_frame()));
        }
        pending.push_back(Ok(Bytes::from(self.anthropic.encode_stream_done())));
    }
}

struct AnthropicStreamState {
    upstream: StreamingResponse,
    buffer: String,
    pending: VecDeque<Result<Bytes, ProviderError>>,
    bridge: AnthropicSseBridge,
}

impl AnthropicStreamState {
    fn new(
        upstream: StreamingResponse,
        openai: OpenAIConverter,
        anthropic: AnthropicConverter,
    ) -> Self {
        Self {
            upstream,
            buffer: String::new(),
            pending: VecDeque::new(),
            bridge: AnthropicSseBridge::new(openai, anthropic),
        }
    }

    fn drain_complete_frames(&mut self) {
        while let Some(end) = self.buffer.find("\n\n") {
            let raw = self.buffer[..end + 2].to_owned();
            self.buffer.drain(..end + 2);
            self.bridge.push_frame_results(&raw, &mut self.pending);
        }
    }

    fn flush_trailing_frame(&mut self) {
        if self.buffer.trim().is_empty() {
            self.buffer.clear();
            return;
        }
        let raw = std::mem::take(&mut self.buffer);
        self.bridge.push_frame_results(&raw, &mut self.pending);
    }
}

fn openai_sse_to_anthropic_sse(
    upstream: StreamingResponse,
    openai: OpenAIConverter,
    anthropic: AnthropicConverter,
) -> StreamingResponse {
    Box::pin(futures::stream::unfold(
        AnthropicStreamState::new(upstream, openai, anthropic),
        |mut state| async move {
            loop {
                if let Some(item) = state.pending.pop_front() {
                    return Some((item, state));
                }

                match state.upstream.next().await {
                    Some(Ok(bytes)) => match std::str::from_utf8(&bytes) {
                        Ok(text) => {
                            state.buffer.push_str(text);
                            state.drain_complete_frames();
                        }
                        Err(err) => {
                            return Some((Err(ProviderError::Decode(err.to_string())), state));
                        }
                    },
                    Some(Err(err)) => return Some((Err(err), state)),
                    None => {
                        state.flush_trailing_frame();
                        return state.pending.pop_front().map(|item| (item, state));
                    }
                }
            }
        },
    ))
}

fn content_block_start_frame() -> Bytes {
    Bytes::from_static(
        b"event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
    )
}

fn content_block_stop_frame() -> Bytes {
    Bytes::from_static(
        b"event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_error_bind_renders_address_and_source() {
        let err = ServerError::Bind {
            addr: ([127, 0, 0, 1], 65535).into(),
            source: std::io::Error::new(std::io::ErrorKind::AddrInUse, "boom"),
        };
        let rendered = format!("{err}");
        assert!(rendered.contains("127.0.0.1:65535"));
        assert!(rendered.contains("boom"));
    }

    #[test]
    fn server_error_no_available_port_renders_range() {
        let err = ServerError::NoAvailablePort {
            start: 8787,
            attempts: 100,
            last: None,
        };
        let rendered = format!("{err}");
        assert!(rendered.contains("8787"));
        assert!(rendered.contains("8886"));
        assert!(
            rendered.contains("All ports occupied"),
            "message must be UI-friendly, got: {rendered}",
        );
    }

    #[tokio::test]
    async fn bind_with_fallback_exhausts_range_returns_ui_friendly_error() {
        // Bind a single port, then ask bind_with_fallback to try only
        // that one port — it must return NoAvailablePort.
        let first = ProxyServer::bind(([127, 0, 0, 1], 0).into())
            .await
            .expect("os-allocated bind");
        let port = first.local_addr().port();
        let err = ProxyServer::bind_with_fallback(port, 1)
            .await
            .expect_err("must fail — port is occupied");
        let msg = format!("{err}");
        assert!(
            msg.contains("All ports occupied"),
            "expected UI-friendly message, got: {msg}",
        );
    }

    #[test]
    fn server_error_port_overflow_renders_start_and_offset() {
        let err = ServerError::PortOverflow {
            start: u16::MAX - 2,
            offset: 5,
        };
        let rendered = format!("{err}");
        assert!(rendered.contains(&(u16::MAX - 2).to_string()));
        assert!(rendered.contains('5'));
    }
}
