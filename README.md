# CCUse

> Local API proxy + transparent multi-provider failover. Configure once, fail over everywhere.

[中文版 / Chinese](./README.zh-CN.md)

---

## Overview

CCUse is a desktop app that exposes a single local HTTP API endpoint and abstracts multiple AI providers (Claude / OpenAI / Gemini / relays / custom endpoints) behind it. Any client that supports a custom API base URL — Claude Desktop, Cursor, Continue, etc. — points at the local proxy and gets:

- One-time config, global effect
- Background health check + automatic failover
- 5 switching strategies — Priority / Smart / Load Balance / Fastest / Cost
- Real-time monitoring and metrics
- Cross-platform — macOS (Apple Silicon + Intel) and Windows

---

## Status

|           |                                                            |
| --------- | ---------------------------------------------------------- |
| Version   | `1.1.2`                                                    |
| Roadmap   | [`开发计划.md`](./docs/开发计划.md)                        |
| Changelog | [`CHANGELOG.md`](./CHANGELOG.md)                           |
| Releases  | [GitHub Releases](https://github.com/colna/CCUse/releases) |

---

## Documentation

| Doc                                                                | Contents                                                                |
| ------------------------------------------------------------------ | ----------------------------------------------------------------------- |
| [`User Manual`](./docs/user-manual.md)                             | Bilingual usage guide — config, strategies, monitoring, import/export   |
| [`FAQ`](./docs/FAQ.md)                                             | Common issues & troubleshooting (port, WebView2, notarization, signing) |
| [`产品技术文档.md`](./docs/产品技术文档.md)                        | Product positioning, architecture, tech stack, DB schema, security      |
| [`开发计划.md`](./docs/开发计划.md)                                | Phase plan — task IDs, estimates, dependencies                          |
| [`CLAUDE.md`](./CLAUDE.md)                                         | Project working rules — tests, task reports, context, design/code rules |
| [`任务报告.md`](./docs/任务报告.md)                                | Append-only execution log                                               |
| [`.github/workflows/release.yml`](./.github/workflows/release.yml) | Version-driven release pipeline (3 platform artifacts)                  |

---

## Repo Layout (monorepo)

```
CCUse/
├── apps/
│   ├── desktop/          # Tauri 2 + React 18 + TS + antd v6
│   │   ├── src/          # React frontend
│   │   └── src-tauri/    # Rust backend
│   └── website/          # Next.js 14 marketing site
├── packages/
│   └── ui/               # Shared shadcn primitives + Tailwind preset
├── .github/workflows/
│   └── release.yml       # Auto tag + 3-platform build + GitHub Release
├── pnpm-workspace.yaml
├── turbo.json            # Turborepo pipeline + Vercel remote cache
└── package.json
```

---

## Quick Start

### Prerequisites

- Node.js ≥ 20
- pnpm ≥ 9 (tested on 10.30.3)
- Rust 1.77+ (with cargo)
- macOS: Xcode Command Line Tools / Windows: MSVC build tools

### Install & Run

```bash
# 1. Install deps
pnpm install

# 2. Run desktop app in dev mode
pnpm dev:desktop

# 3. Run website in dev mode
pnpm dev:website

# 4. Tests
pnpm test:desktop                                  # Vitest (React)
cd apps/desktop/src-tauri && cargo test            # cargo test (Rust)

# 5. Type check
pnpm typecheck
```

### Verify the local API

After adding and enabling at least one provider in the desktop app, copy the **OpenAI** Base URL and protocol-scoped local API key from the Dashboard. OpenAI-compatible clients use a Base URL ending in `/v1`, for example `http://127.0.0.1:8787/v1`; Anthropic clients use the separate **Anthropic** Base URL/key pair shown beside it.

```bash
curl -sS http://127.0.0.1:8787/v1/chat/completions \
  -H "Authorization: Bearer sk-local-..." \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-5.4",
    "messages": [
      { "role": "user", "content": "Reply with: CCUse OK" }
    ],
    "stream": false
  }'
```

A successful call returns an OpenAI-compatible `chat.completion` JSON and the dashboard shows a new request log entry. If you see `providers_not_configured`, double-check at least one provider is enabled.

> Phases 1.0.1 → 1.0.6 + 1.0.W are complete. Local proxy, multi-provider management, automatic failover, format conversion, monitoring dashboard, system tray, i18n, and the Next.js marketing website are all in place. See [`开发计划.md`](./docs/开发计划.md).

---

## Supported Local API Endpoints

| Method | Path                   | Compatible with                 |
| ------ | ---------------------- | ------------------------------- |
| GET    | `/healthz`             | health check (no auth)          |
| GET    | `/v1/models`           | OpenAI Models API               |
| POST   | `/v1/chat/completions` | OpenAI Chat Completions (+ SSE) |
| POST   | `/v1/messages`         | Anthropic Messages (+ SSE)      |

**Auth** — OpenAI-compatible routes use the OpenAI key with `Authorization: Bearer sk-local-…`; Anthropic Messages uses the Anthropic key with `x-api-key: sk-local-…`. CORS is restricted to localhost.

---

## Development Workflow

> Any work in this repo must first read [`CLAUDE.md`](./CLAUDE.md) — it defines mandatory test requirements, task report format, context-management thresholds, and design/code best-practices skill order.

Quick reference:

- **Tests required for every feature** — Rust unit + integration, React component, key-flow E2E
- **After each task** — append [`任务报告.md`](./docs/任务报告.md) → check context → `/compact` if > 40%
- **UI/UX** — call `/apple-design` then `/frontend-design` before coding
- **Code best-practices** — Rust → `/rust-best-practices`; React → `/vercel-react-best-practices`; Next.js → `/next-best-practices`
- **Styling** — Tailwind utility + antd v6 `ConfigProvider` tokens only; no CSS Modules / styled-components / Sass / global CSS (except `globals.css` entry)
- **Commits** — one commit per task with the task ID in the message; long-term authorized to commit + push (effective 2026-04-28)

---

## Release

Version-driven, fully automated pipeline.

1. Bump `version` in `apps/desktop/src-tauri/tauri.conf.json`
2. Merge to `main`
3. GitHub Actions: detect version → tag `v<version>` → 3-platform matrix build → GitHub Release

| `version` | Release type          | Artifacts                                         |
| --------- | --------------------- | ------------------------------------------------- |
| `0.0.0`   | Skipped (placeholder) | —                                                 |
| `0.x.y`   | Pre-release           | `*_aarch64.dmg` / `*_x64.dmg` / `*_x64-setup.exe` |
| `1.0.0+`  | Stable                | Same as above                                     |

### macOS "CCUse is damaged"

CI does not yet enable Apple Developer notarization, so dmg downloads from a browser may be quarantined by Gatekeeper as "damaged". **It is not actually damaged.** Once you've confirmed the dmg comes from [GitHub Releases](https://github.com/colna/CCUse/releases), drag `CCUse.app` into `/Applications` and run once:

```bash
sudo xattr -dr com.apple.quarantine /Applications/CCUse.app
```

Details: [`docs/faq-troubleshooting`](./apps/website/content/docs/zh/faq-troubleshooting.mdx).

---

## License

TBD.
