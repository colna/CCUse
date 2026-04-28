//! SSE frame parsing utilities (shared across converters).
//!
//! Extracts `data:` payloads from raw SSE byte streams, handling
//! multi-line data fields and event types.

/// Parse a raw SSE text block into individual `(event_type, data)` pairs.
///
/// The SSE spec allows:
/// - `event: <type>\n`
/// - `data: <payload>\n`
/// - blank lines as event separators
///
/// This function yields one entry per complete event.
#[must_use]
pub fn parse_sse_frames(raw: &str) -> Vec<SseFrame> {
    let mut frames = Vec::new();
    let mut current_event = None;
    let mut current_data = String::new();

    for line in raw.lines() {
        if line.is_empty() {
            // Blank line = event boundary.
            if !current_data.is_empty() {
                frames.push(SseFrame {
                    event: current_event.take(),
                    data: std::mem::take(&mut current_data),
                });
            }
            current_event = None;
            continue;
        }

        if let Some(rest) = line.strip_prefix("event:") {
            current_event = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("data:") {
            if !current_data.is_empty() {
                current_data.push('\n');
            }
            current_data.push_str(rest.trim());
        }
        // Ignore other fields (id:, retry:, comments).
    }

    // Trailing event without final blank line.
    if !current_data.is_empty() {
        frames.push(SseFrame {
            event: current_event.take(),
            data: current_data,
        });
    }

    frames
}

/// A single parsed SSE frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseFrame {
    pub event: Option<String>,
    pub data: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_openai_style() {
        let raw = "data: {\"id\":\"1\"}\n\ndata: {\"id\":\"2\"}\n\ndata: [DONE]\n\n";
        let frames = parse_sse_frames(raw);
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].data, "{\"id\":\"1\"}");
        assert_eq!(frames[2].data, "[DONE]");
        assert!(frames[0].event.is_none());
    }

    #[test]
    fn parse_anthropic_style() {
        let raw = "\
event: message_start\n\
data: {\"type\":\"message_start\"}\n\
\n\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\"}\n\
\n\
event: message_stop\n\
data: {\"type\":\"message_stop\"}\n\n";
        let frames = parse_sse_frames(raw);
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].event.as_deref(), Some("message_start"));
        assert_eq!(frames[1].event.as_deref(), Some("content_block_delta"));
    }

    #[test]
    fn multi_line_data() {
        let raw = "data: line1\ndata: line2\n\n";
        let frames = parse_sse_frames(raw);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data, "line1\nline2");
    }

    #[test]
    fn empty_input() {
        assert!(parse_sse_frames("").is_empty());
    }

    #[test]
    fn trailing_event_no_blank() {
        let raw = "data: last";
        let frames = parse_sse_frames(raw);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data, "last");
    }
}
