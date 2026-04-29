//! T1.0.5.02 — Memory baseline regression tests.
//!
//! Uses `std::mem::size_of` to pin the in-memory footprint of core domain
//! types. If someone adds a large field (e.g. an extra `Vec` or `HashMap`),
//! these assertions will break and force a conscious decision.
//!
//! Expected baselines (64-bit targets):
//!
//! | Type                    | Expected bytes | Notes                                 |
//! |-------------------------|----------------|---------------------------------------|
//! | `Provider`              | <= 224         | 7 Strings + 2 Option + i32 + bool     |
//! | `ProviderInput`         | <= 176         | 4 Strings + enums + Options            |
//! | `ProviderKind`          |   1            | 5-variant enum, fits in 1 byte         |
//! | `SwitchStrategy`        |   1            | 5-variant enum                         |
//! | `SwitchConfig`          | <= 48          | enum + usize + `SmartWeights`          |
//! | `SmartWeights`          |  32            | 4 x f64                                |
//! | `ApiRequest`            | <= 104         | String + Vecs + 2 Option + bool        |
//! | `ApiResponse`           | <= 104         | 2 String + Vec + Option                |
//! | `ChatMessage`           | <= 48          | 2 Strings                              |
//! | `HealthStatus`          |   1            | 3-variant enum                         |
//! | `UnifiedRequest`        | <= 152         | Strings + Vecs + Options               |
//! | `UnifiedResponse`       | <= 104         | 2 Strings + Vec + Option               |
//! | `UnifiedMessage`        | <= 56          | Role + Vec + Option                    |
//! | `ContentPart`           | <= 80          | tagged enum, largest variant           |
//! | `ToolCall`              | <= 72          | 3 Strings                              |
//! | `ToolDefinition`        | <= 80          | 2 Strings + serde Value                |
//! | `SwitchHistoryEntry`    | <= 184         | i64 + 4 Strings + Option + i32         |
//! | `RequestLogEntry`       | <= 224         | i64 + 4 Strings + Options + i64s       |
//!
//! If a test fails because the type grew, evaluate whether the growth is
//! intentional. If so, update the bound; if not, reconsider the change.

use std::mem::size_of;

use ccuse_desktop_lib::converter::{
    ContentPart, Role, ToolCall, ToolDefinition, ToolResult, UnifiedChoice, UnifiedMessage,
    UnifiedRequest, UnifiedResponse, UnifiedStreamChunk, UnifiedUsage,
};
use ccuse_desktop_lib::providers::model::{Provider, ProviderInput, ProviderKind};
use ccuse_desktop_lib::providers::{
    ApiChoice, ApiRequest, ApiResponse, ApiUsage, ChatMessage, HealthStatus,
};
use ccuse_desktop_lib::switch::strategy::{SmartWeights, SwitchStrategy};
use ccuse_desktop_lib::switch::{
    RequestLogEntry, RequestLogInput, SwitchConfig, SwitchHistoryEntry, SwitchHistoryInput,
};

// ── Provider domain ────────────────────────────────────────────────

#[test]
fn provider_kind_is_one_byte() {
    assert_eq!(size_of::<ProviderKind>(), 1);
}

#[test]
fn provider_input_fits_in_176_bytes() {
    let size = size_of::<ProviderInput>();
    assert!(
        size <= 176,
        "ProviderInput grew to {size} bytes (limit 176)"
    );
}

#[test]
fn provider_fits_in_224_bytes() {
    let size = size_of::<Provider>();
    assert!(size <= 224, "Provider grew to {size} bytes (limit 224)");
}

// ── Switch engine ──────────────────────────────────────────────────

#[test]
fn switch_strategy_is_one_byte() {
    assert_eq!(size_of::<SwitchStrategy>(), 1);
}

#[test]
fn smart_weights_is_32_bytes() {
    assert_eq!(size_of::<SmartWeights>(), 32);
}

#[test]
fn switch_config_fits_in_48_bytes() {
    let size = size_of::<SwitchConfig>();
    assert!(size <= 48, "SwitchConfig grew to {size} bytes (limit 48)");
}

// ── API types ──────────────────────────────────────────────────────

#[test]
fn chat_message_fits_in_48_bytes() {
    let size = size_of::<ChatMessage>();
    assert!(size <= 48, "ChatMessage grew to {size} bytes (limit 48)");
}

#[test]
fn api_request_fits_in_104_bytes() {
    let size = size_of::<ApiRequest>();
    assert!(size <= 104, "ApiRequest grew to {size} bytes (limit 104)");
}

#[test]
fn api_response_fits_in_104_bytes() {
    let size = size_of::<ApiResponse>();
    assert!(size <= 104, "ApiResponse grew to {size} bytes (limit 104)");
}

#[test]
fn api_choice_fits_in_80_bytes() {
    let size = size_of::<ApiChoice>();
    assert!(size <= 80, "ApiChoice grew to {size} bytes (limit 80)");
}

#[test]
fn api_usage_is_12_bytes() {
    assert_eq!(size_of::<ApiUsage>(), 12);
}

#[test]
fn health_status_is_one_byte() {
    assert_eq!(size_of::<HealthStatus>(), 1);
}

// ── Converter types ────────────────────────────────────────────────

#[test]
fn role_is_one_byte() {
    assert_eq!(size_of::<Role>(), 1);
}

#[test]
fn unified_request_fits_in_152_bytes() {
    let size = size_of::<UnifiedRequest>();
    assert!(
        size <= 152,
        "UnifiedRequest grew to {size} bytes (limit 152)"
    );
}

#[test]
fn unified_response_fits_in_104_bytes() {
    let size = size_of::<UnifiedResponse>();
    assert!(
        size <= 104,
        "UnifiedResponse grew to {size} bytes (limit 104)"
    );
}

#[test]
fn unified_message_fits_in_56_bytes() {
    let size = size_of::<UnifiedMessage>();
    assert!(size <= 56, "UnifiedMessage grew to {size} bytes (limit 56)");
}

#[test]
fn content_part_fits_in_80_bytes() {
    let size = size_of::<ContentPart>();
    assert!(size <= 80, "ContentPart grew to {size} bytes (limit 80)");
}

#[test]
fn tool_call_fits_in_72_bytes() {
    let size = size_of::<ToolCall>();
    assert!(size <= 72, "ToolCall grew to {size} bytes (limit 72)");
}

#[test]
fn tool_result_fits_in_48_bytes() {
    let size = size_of::<ToolResult>();
    assert!(size <= 48, "ToolResult grew to {size} bytes (limit 48)");
}

#[test]
fn tool_definition_fits_in_80_bytes() {
    let size = size_of::<ToolDefinition>();
    assert!(size <= 80, "ToolDefinition grew to {size} bytes (limit 80)");
}

#[test]
fn unified_choice_fits_in_64_bytes() {
    let size = size_of::<UnifiedChoice>();
    assert!(size <= 64, "UnifiedChoice grew to {size} bytes (limit 64)");
}

#[test]
fn unified_usage_is_12_bytes() {
    assert_eq!(size_of::<UnifiedUsage>(), 12);
}

#[test]
fn unified_stream_chunk_fits_in_104_bytes() {
    let size = size_of::<UnifiedStreamChunk>();
    assert!(
        size <= 104,
        "UnifiedStreamChunk grew to {size} bytes (limit 104)"
    );
}

// ── Switch audit types ─────────────────────────────────────────────

#[test]
fn switch_history_entry_fits_in_184_bytes() {
    let size = size_of::<SwitchHistoryEntry>();
    assert!(
        size <= 184,
        "SwitchHistoryEntry grew to {size} bytes (limit 184)"
    );
}

#[test]
fn switch_history_input_fits_in_152_bytes() {
    let size = size_of::<SwitchHistoryInput>();
    assert!(
        size <= 152,
        "SwitchHistoryInput grew to {size} bytes (limit 152)"
    );
}

#[test]
fn request_log_entry_fits_in_224_bytes() {
    let size = size_of::<RequestLogEntry>();
    assert!(
        size <= 224,
        "RequestLogEntry grew to {size} bytes (limit 224)"
    );
}

#[test]
fn request_log_input_fits_in_200_bytes() {
    let size = size_of::<RequestLogInput>();
    assert!(
        size <= 200,
        "RequestLogInput grew to {size} bytes (limit 200)"
    );
}

// ── Summary assertion ──────────────────────────────────────────────

#[test]
fn print_all_sizes_for_reference() {
    println!("=== CCUse type size baseline (bytes, 64-bit) ===");
    println!("ProviderKind:         {}", size_of::<ProviderKind>());
    println!("ProviderInput:        {}", size_of::<ProviderInput>());
    println!("Provider:             {}", size_of::<Provider>());
    println!("SwitchStrategy:       {}", size_of::<SwitchStrategy>());
    println!("SmartWeights:         {}", size_of::<SmartWeights>());
    println!("SwitchConfig:         {}", size_of::<SwitchConfig>());
    println!("ChatMessage:          {}", size_of::<ChatMessage>());
    println!("ApiRequest:           {}", size_of::<ApiRequest>());
    println!("ApiResponse:          {}", size_of::<ApiResponse>());
    println!("ApiChoice:            {}", size_of::<ApiChoice>());
    println!("ApiUsage:             {}", size_of::<ApiUsage>());
    println!("HealthStatus:         {}", size_of::<HealthStatus>());
    println!("Role:                 {}", size_of::<Role>());
    println!("UnifiedRequest:       {}", size_of::<UnifiedRequest>());
    println!("UnifiedResponse:      {}", size_of::<UnifiedResponse>());
    println!("UnifiedMessage:       {}", size_of::<UnifiedMessage>());
    println!("ContentPart:          {}", size_of::<ContentPart>());
    println!("ToolCall:             {}", size_of::<ToolCall>());
    println!("ToolResult:           {}", size_of::<ToolResult>());
    println!("ToolDefinition:       {}", size_of::<ToolDefinition>());
    println!("UnifiedChoice:        {}", size_of::<UnifiedChoice>());
    println!("UnifiedUsage:         {}", size_of::<UnifiedUsage>());
    println!("UnifiedStreamChunk:   {}", size_of::<UnifiedStreamChunk>());
    println!("SwitchHistoryEntry:   {}", size_of::<SwitchHistoryEntry>());
    println!("SwitchHistoryInput:   {}", size_of::<SwitchHistoryInput>());
    println!("RequestLogEntry:      {}", size_of::<RequestLogEntry>());
    println!("RequestLogInput:      {}", size_of::<RequestLogInput>());
}
