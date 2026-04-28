//! Format conversion layer (Phase 1.0.3).
//!
//! Translates between vendor-specific wire formats (`OpenAI`, Anthropic,
//! Gemini) and the [`types::UnifiedRequest`] / [`types::UnifiedResponse`]
//! intermediate representation.

pub mod anthropic;
pub mod gemini;
pub mod model_mapping;
pub mod openai;
pub mod sse;
pub mod types;

use thiserror::Error;

pub use anthropic::AnthropicConverter;
pub use gemini::GeminiConverter;
pub use model_mapping::ModelMapping;
pub use openai::OpenAIConverter;
pub use types::{
    ContentPart, FinishReason, Role, StreamChoice, StreamDelta, StreamToolCall, ToolCall,
    ToolDefinition, ToolResult, UnifiedChoice, UnifiedMessage, UnifiedRequest, UnifiedResponse,
    UnifiedStreamChunk, UnifiedUsage,
};

/// Errors that occur during format conversion.
#[derive(Debug, Error)]
pub enum ConvertError {
    #[error("missing required field: {0}")]
    MissingField(String),
    #[error("unsupported content type: {0}")]
    UnsupportedContent(String),
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid role: {0}")]
    InvalidRole(String),
}

/// Trait implemented by each vendor converter.
pub trait FormatConverter {
    /// Parse a vendor-specific JSON request body into unified format.
    fn request_to_unified(&self, body: &serde_json::Value) -> Result<UnifiedRequest, ConvertError>;

    /// Serialise a unified request into the vendor-specific JSON body.
    fn unified_to_request(&self, req: &UnifiedRequest) -> Result<serde_json::Value, ConvertError>;

    /// Parse a vendor-specific JSON response body into unified format.
    fn response_to_unified(
        &self,
        body: &serde_json::Value,
    ) -> Result<UnifiedResponse, ConvertError>;

    /// Serialise a unified response into the vendor-specific JSON body.
    fn unified_to_response(
        &self,
        resp: &UnifiedResponse,
    ) -> Result<serde_json::Value, ConvertError>;

    /// Parse a single vendor-specific SSE data payload into a unified
    /// stream chunk.  Returns `None` for terminal events (e.g. `[DONE]`).
    fn parse_stream_chunk(
        &self,
        data: &str,
    ) -> Result<Option<UnifiedStreamChunk>, ConvertError>;

    /// Encode a unified stream chunk into vendor-specific SSE frame(s).
    fn encode_stream_chunk(&self, chunk: &UnifiedStreamChunk) -> Result<String, ConvertError>;

    /// Encode the terminal SSE frame for this vendor format.
    fn encode_stream_done(&self) -> String;
}
