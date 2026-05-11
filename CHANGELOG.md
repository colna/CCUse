# Changelog

All notable changes to CCUse will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.1.0] - 2026-05-11

### Added

- Migrated desktop UI to **Ant Design v6** with Apple-flavored design tokens and system color sync; Dashboard / LocalApiCard / shell, Providers / Settings / Strategy pages now share the new look (T-A, T-B, T-C).
- New marketing **website** (`apps/website`, Next.js App Router) with locale routing, theme switching, SEO baseline, hero / feature cards / architecture sections, MDX-powered docs (sidebar + TOC + search), getting-started / strategy / monitoring / FAQ guides, features page, and a download page that fetches the latest GitHub release with platform-aware recommendation cards.
- Shared `@ccuse/ui` package exposes shadcn primitives reused by `apps/website` and `apps/desktop`.
- Added **Tauri WebDriver E2E** smoke flow under `apps/desktop`'s Playwright config.
- Provider list now shows loading states for create / update / delete operations, and "Add provider" form collapses by default.

### Changed

- Provider dispatch attempts are now logged through the proxy runtime, surfacing into Dashboard switch history.
- Health check / failover / smart-strategy weighting reworked: status events push to UI in real time, weights stay balanced, fastest-overview refresh aligned with actual switch logic.
- Dropped `lucide-react` dependency; icons are unified via `@ant-design/icons`.

### Fixed

- antd CSS variables now bind to Tailwind tokens, fixing light/dark theme inversion via `:root --app-*` tokens.
- Provider kind selection state is now visually obvious; provider api key, type, and other fields are editable; deleting a provider no longer fails when switch history references it.
- Proxy correctness:
  - extend provider timeout to 600s for long completions (T1.0.6.19);
  - preserve inbound model candidates and skip upstream model rewriting (T1.0.6.16);
  - preserve Anthropic tool results and harden message ids / usage defaults (T1.0.6.09 / T1.0.6.10);
  - align Anthropic Claude headers and route native Anthropic providers correctly;
  - align exhausted-account errors with sub2api shape (T1.0.6.03);
  - surface provider network error causes and connection-probe failures in dialog (T1.0.4.05);
  - preserve multimodal provider requests (T1.0.3.02);
  - add provider default model fallback;
  - align health-check model ids with cc-switch streaming probe.
- Avoid macOS keychain prompt on startup; replace unreadable provider keys (T1.0.2.19).
- Unify project / tray / Windows icons (T1.0.6.31).

### CI

- Release workflow detects existing assets via `jq any` and resumes incomplete releases instead of skipping;
- Optional Apple / Windows signing secrets are gated behind `CCUSE_APPLE_SIGNING_ENABLED` / `CCUSE_WINDOWS_SIGNING_ENABLED` repository variables; disabled signing no longer leaks empty env vars into the build.

## [1.0.1] - 2026-04-29

### Fixed

- `fix(proxy): wire /v1/* HTTP routes to SwitchEngine` — the local API routes now dispatch through configured providers instead of returning the Phase 1.0.1 `providers_not_configured` stub whenever providers exist.
- Provider CRUD now hot-reloads the runtime `ProviderManager`, so add/update/delete changes affect the next proxy request without restarting the app.
- Failover after retriable upstream errors records degraded provider state and writes switch history for the dashboard timeline.

### Added

User-visible local API endpoint coverage:

- `GET /v1/models` aggregates models from enabled providers, namespaces ids as `provider_id::model_id`, caches results for 30 seconds, and returns partial results when one provider fails.
- `POST /v1/chat/completions` supports OpenAI-compatible non-streaming and streaming chat completions, model mapping, function tools, tool result messages, request logging, and automatic provider failover.
- `POST /v1/messages` supports Anthropic Messages-compatible non-streaming and streaming responses, `system`, `stop_sequences`, `tool_use`, `tool_result`, Anthropic-shaped error envelopes, and Anthropic SSE events.

Documentation updates:

- Added a bilingual supported-endpoints matrix to the user manual.
- Added a copyable README `curl` quick check for `/v1/chat/completions`.

### Changed

- Non-streaming `/v1/*` handlers now enforce a 1 MiB body limit and a 60 second default timeout; streaming responses are not cut off by that non-streaming timeout.
- Proxy runtime monitoring now writes both request logs and switch history, enabling dashboard metrics after real local API calls.

## [1.0.0] - 2026-04-28

### Added

**Local Proxy (Phase 1.0.1)**

- Local HTTP API proxy with automatic port selection (100-port fallback)
- Bearer token authentication with `sk-local-*` API keys
- CORS restriction (localhost-only)
- `/healthz` endpoint for health checks
- `/v1/models`, `/v1/chat/completions`, `/v1/messages` routing

**Provider Management (Phase 1.0.2)**

- CRUD for 5 provider types: OpenAI, Anthropic, Gemini, Relay, Custom
- AES-256-GCM API key encryption with OS keyring master key
- File-based keyring fallback when OS keyring unavailable
- 5 switching strategies: Priority, Smart, Load Balance, Fastest, Cost
- Background health checking with configurable intervals
- Provider connection testing

**Format Conversion (Phase 1.0.3)**

- Unified internal format bridging OpenAI, Anthropic, and Gemini APIs
- Request/response/streaming conversion across all 3 formats (3x3x2 matrix)
- Tool calling cross-format translation
- Model name mapping with user-configurable overrides
- SSE stream chunk encoding/decoding

**Desktop UI (Phase 1.0.4)**

- Dashboard with real-time status cards, charts, and metrics
- Success rate and latency trend charts (24h, 5-min buckets)
- Cost distribution pie chart by provider
- Switch event timeline
- Drag-and-drop provider reordering
- Inline provider editing with live validation
- Strategy configuration with advanced parameter tuning
- Model mapping management UI
- Configuration export/import with password encryption (scrypt + AES-256-GCM)
- 3 template presets (Claude, OpenAI, Gemini)
- System tray with status, copy API key, restart proxy, quit
- Close-to-tray (window hide instead of quit)
- Desktop notifications via OS notification center
- Chinese/English i18n with system language detection
- Language switcher with localStorage persistence

**Stability & Security (Phase 1.0.5)**

- Port conflict graceful degradation (100-port retry)
- OS keyring failure fallback to encrypted file store
- SQLite WAL mode with busy timeout for concurrent access
- Panic safety net with crash log persistence
- Content Security Policy (no `unsafe-eval`)
- Log sanitization audit (no API keys in logs)
- Dependency security audit

### Security

- API keys encrypted at rest (AES-256-GCM)
- Master key in OS keyring (macOS Keychain / Windows Credential Manager)
- File-based fallback with 0600 permissions when keyring unavailable
- CSP: `default-src 'self'`, no `unsafe-eval`
- Config export files encrypted with user password (scrypt KDF)
- No sensitive data in application logs (enforced by automated test)

### Documentation

- Bilingual user manual (Chinese/English)
- FAQ covering common issues (port conflicts, WebView2, notarization, SmartScreen)
- Updated README with feature overview and quick start guide

## [0.4.0] - 2026-04-28

### Added

- Dashboard with status cards, charts, and metrics (T1.0.4.01-14)
- System tray with context menu (T1.0.4.15-16)
- Desktop notifications (T1.0.4.17)
- Config export/import with encryption (T1.0.4.18-20)
- i18n with Chinese/English support (T1.0.4.21-23)
- Close-to-tray behavior (T1.0.4.24)

## [0.3.0] - 2026-04-28

### Added

- Unified format converter for OpenAI/Anthropic/Gemini (T1.0.3.01-11)
- Model name mapping with configurable overrides (T1.0.3.12)
- 46 integration tests covering 3x3x2 compatibility matrix (T1.0.3.13-14)

## [0.2.0] - 2026-04-28

### Added

- Provider CRUD with encrypted API key storage (T1.0.2.19)
- Switch engine with 5 strategies (T1.0.2.20)
- Health checker with periodic polling (T1.0.2.21)

## [0.1.0] - 2026-04-28

### Added

- Local HTTP proxy server with port auto-selection
- Bearer token authentication
- Provider trait and repository with SQLite persistence
- React UI shell with sidebar navigation
- Automated release pipeline (GitHub Actions)
