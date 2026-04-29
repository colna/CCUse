//! SSE response helpers for the proxy.
//!
//! `Provider::send_stream_request` returns raw upstream SSE bytes
//! (`data: {...}\n\n`); the proxy forwards them verbatim with the
//! right HTTP headers. We avoid `axum::response::sse::Sse` because
//! it expects pre-parsed `Event`s — re-parsing every chunk would
//! double the work and risk altering the wire shape clients see.
//!
//! Keep-alive is bolted on via [`with_keep_alive`]: when the upstream
//! is silent for too long, we slip in `: keepalive\n\n` so middlebox
//! / browser timeouts don't kill the connection. SSE comments
//! (`:` prefix) are ignored by every conformant client.

use std::time::Duration;

use axum::body::Body;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use futures::stream::{Stream, StreamExt};

use crate::providers::api::{ProviderError, StreamingResponse};

/// Default keep-alive ping cadence. 15 s sits well under typical
/// idle-timeout floors (NGINX 60 s, Cloudflare 100 s).
pub const DEFAULT_KEEP_ALIVE: Duration = Duration::from_secs(15);

/// Wire bytes used as the keep-alive frame. Comment line per the
/// SSE spec — clients that don't know about it ignore it without
/// surfacing anything to the user.
const KEEP_ALIVE_FRAME: &[u8] = b": keepalive\n\n";

/// Wrap [`StreamingResponse`] with periodic keep-alive comments so
/// idle connections don't get reaped. `interval` is the *maximum
/// silence* we'll tolerate before injecting a frame; it's not a
/// strict tick — actual upstream chunks reset the clock implicitly
/// because we still forward them at line-rate.
pub fn with_keep_alive(
    upstream: StreamingResponse,
    interval: Duration,
) -> impl Stream<Item = Result<Bytes, ProviderError>> + Send {
    let ticker = tokio::time::interval(interval);
    futures::stream::unfold(
        (upstream, ticker, false),
        |(mut upstream, mut ticker, first_tick_seen)| async move {
            let mut skip_immediate_tick = first_tick_seen;
            loop {
                tokio::select! {
                    item = upstream.next() => {
                        return item.map(|chunk| (chunk, (upstream, ticker, skip_immediate_tick)));
                    }
                    _ = ticker.tick() => {
                        if skip_immediate_tick {
                            return Some((
                                Ok(Bytes::from_static(KEEP_ALIVE_FRAME)),
                                (upstream, ticker, skip_immediate_tick),
                            ));
                        }
                        skip_immediate_tick = true;
                    }
                }
            }
        },
    )
}

/// Convert a byte stream into an HTTP response with the SSE
/// content-type and disabled caching. Used by `chat_completions`
/// once T1.0.2.15 routes streams through `SwitchEngine`.
pub fn stream_to_sse_response<S>(stream: S) -> Response
where
    S: Stream<Item = Result<Bytes, ProviderError>> + Send + 'static,
{
    let body = Body::from_stream(
        stream.map(|item| item.map_err(|e| std::io::Error::other(e.to_string()))),
    );

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    // NGINX-isms: tell intermediaries not to buffer the stream so
    // chunks reach the browser as they're produced.
    headers.insert(
        HeaderName::from_static("x-accel-buffering"),
        HeaderValue::from_static("no"),
    );

    (StatusCode::OK, headers, body).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use futures::stream;

    #[tokio::test]
    async fn stream_to_sse_response_sets_event_stream_headers() {
        let s = stream::iter(vec![Ok::<Bytes, ProviderError>(Bytes::from_static(
            b"data: hi\n\n",
        ))]);
        let response = stream_to_sse_response(s);
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(CONTENT_TYPE).unwrap(),
            "text/event-stream",
        );
        assert_eq!(response.headers().get(CACHE_CONTROL).unwrap(), "no-cache");
        assert_eq!(response.headers().get("x-accel-buffering").unwrap(), "no",);
    }

    #[tokio::test]
    async fn keep_alive_injects_comment_when_upstream_idle() {
        // Upstream is empty (no chunks). Keep-alive must still emit
        // at least one frame after the first interval tick.
        let upstream: StreamingResponse =
            Box::pin(stream::pending::<Result<Bytes, ProviderError>>());
        let wrapped = with_keep_alive(upstream, Duration::from_millis(20));
        futures::pin_mut!(wrapped);
        let chunk = tokio::time::timeout(Duration::from_secs(1), wrapped.next())
            .await
            .expect("must produce keep-alive within 1s")
            .expect("stream must not end")
            .expect("ok variant");
        assert_eq!(chunk, Bytes::from_static(KEEP_ALIVE_FRAME));
    }

    #[tokio::test]
    async fn keep_alive_passes_upstream_chunks_through() {
        let upstream: StreamingResponse = Box::pin(stream::iter(vec![
            Ok::<Bytes, ProviderError>(Bytes::from_static(b"data: 1\n\n")),
            Ok(Bytes::from_static(b"data: 2\n\n")),
        ]));
        let wrapped = with_keep_alive(upstream, Duration::from_secs(60));
        let chunks: Vec<Bytes> = wrapped
            .filter_map(|item| async move { item.ok() })
            .take(2)
            .collect()
            .await;
        assert_eq!(chunks[0], Bytes::from_static(b"data: 1\n\n"));
        assert_eq!(chunks[1], Bytes::from_static(b"data: 2\n\n"));
    }

    #[tokio::test]
    async fn keep_alive_ends_when_upstream_ends() {
        let upstream: StreamingResponse = Box::pin(stream::iter(vec![Ok::<Bytes, ProviderError>(
            Bytes::from_static(b"data: [DONE]\n\n"),
        )]));
        let wrapped = with_keep_alive(upstream, Duration::from_secs(60));
        let chunks: Vec<Bytes> = wrapped
            .filter_map(|item| async move { item.ok() })
            .collect()
            .await;
        assert_eq!(chunks, vec![Bytes::from_static(b"data: [DONE]\n\n")]);
    }
}
