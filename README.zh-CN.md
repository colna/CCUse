# CCUse

> 本地 API 代理 + 多供应商无感切换 —— 一次配置，全局自动故障转移。

[English / 英文版](./README.md)

---

## 概述

CCUse 是一款桌面应用，在本地启动统一的 HTTP API 代理服务，将多个 AI 服务供应商（Claude / OpenAI / Gemini / 中转商 / 自定义端点）抽象为单一接口。任何支持自定义 API 的客户端（Claude Desktop、Cursor、Continue 等）只需指向本地代理，即可获得：

- 单次配置 → 全局生效
- 后台健康检查 + 自动故障转移
- 五种切换策略（优先级 / 智能 / 负载均衡 / 最快响应 / 成本优先）
- 实时监控与统计
- 跨平台（macOS Apple Silicon / Intel + Windows）

---

## 当前状态

|           |                                                            |
| --------- | ---------------------------------------------------------- |
| 版本      | `1.1.2`                                                    |
| 路线图    | [`开发计划.md`](./docs/开发计划.md)                        |
| CHANGELOG | [`CHANGELOG.md`](./CHANGELOG.md)                           |
| 发布      | [GitHub Releases](https://github.com/colna/CCUse/releases) |

---

## 文档索引

| 文档                                                               | 内容                                                                        |
| ------------------------------------------------------------------ | --------------------------------------------------------------------------- |
| [`用户手册`](./docs/user-manual.md)                                | 配置、切换策略、监控、导入导出使用说明（中英双语）                          |
| [`FAQ`](./docs/FAQ.md)                                             | 常见问题与排错（端口冲突、WebView2、公证、签名等）                          |
| [`产品技术文档.md`](./docs/产品技术文档.md)                        | 产品定位、形态、功能、架构、技术选型、数据库设计、安全性                    |
| [`开发计划.md`](./docs/开发计划.md)                                | 5 个 Phase + 官网 Phase，所有 task 按 ID / 工时 / 依赖列出                  |
| [`CLAUDE.md`](./CLAUDE.md)                                         | 项目工作规则（测试、任务报告、context、设计 / 代码 best-practices skill）   |
| [`任务报告.md`](./docs/任务报告.md)                                | append-only 任务执行记录                                                    |
| [`.github/workflows/release.yml`](./.github/workflows/release.yml) | 自动化发布：版本号驱动，三平台产物（aarch64.dmg / x64.dmg / x64-setup.exe） |

---

## 仓库结构（monorepo）

```
CCUse/
├── apps/
│   ├── desktop/          # Tauri 2 + React 18 + TS + antd v6
│   │   ├── src/          # React 前端
│   │   └── src-tauri/    # Rust 后端
│   └── website/          # Next.js 14 官网
├── packages/
│   └── ui/               # 跨端共享 shadcn 组件 + Tailwind preset
├── .github/workflows/
│   └── release.yml       # 自动 tag + 三平台构建 + GitHub Release
├── pnpm-workspace.yaml
├── turbo.json            # Turborepo pipeline + Vercel remote cache
└── package.json
```

---

## 快速开始

### 环境要求

- Node.js ≥ 20
- pnpm ≥ 9（实测 10.30.3）
- Rust 1.77+（含 cargo）
- macOS：Xcode Command Line Tools / Windows：MSVC build tools

### 安装与启动

```bash
# 1. 安装依赖
pnpm install

# 2. 启动桌面端 dev
pnpm dev:desktop

# 3. 启动官网 dev
pnpm dev:website

# 4. 跑测试
pnpm test:desktop                                  # Vitest（React 端）
cd apps/desktop/src-tauri && cargo test            # cargo test（Rust 端）

# 5. 类型检查
pnpm typecheck
```

### 本地 API 快速验证

先在桌面端添加并启用至少一个供应商，然后从仪表盘复制 Base URL 与本地 API Key。将下面的 `sk-local-...` 替换成仪表盘显示的 key；如果代理端口不是 `8787`，也同步替换 URL。

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

成功时返回 OpenAI 兼容的 `chat.completion` JSON，仪表盘会出现一条新的请求记录；若返回 `providers_not_configured`，请确认至少有一个启用的供应商。

> Phase 1.0.1 → 1.0.6 + 官网 1.0.W 已完成。本地代理 + 多供应商管理 + 自动切换 + 格式转换 + 监控面板 + 系统托盘 + i18n + Next.js 官网均已就绪。详见 [`开发计划.md`](./docs/开发计划.md)。

---

## 已支持本地 API 端点

| 方法 | 路径                   | 兼容                            |
| ---- | ---------------------- | ------------------------------- |
| GET  | `/healthz`             | 健康检查（无需鉴权）            |
| GET  | `/v1/models`           | OpenAI Models API               |
| POST | `/v1/chat/completions` | OpenAI Chat Completions (+ SSE) |
| POST | `/v1/messages`         | Anthropic Messages (+ SSE)      |

**鉴权** —— OpenAI 风格 `Authorization: Bearer sk-local-…` 或 Anthropic 风格 `x-api-key: sk-local-…`；CORS 仅放行 localhost。

---

## 开发流程

> 任何在本仓库内的工作，都必须先读 [`CLAUDE.md`](./CLAUDE.md) —— 包含强制的测试要求、任务报告格式、context 管理阈值、设计 / 代码 best-practices skill 调用顺序。

要点速览：

- **每个功能必有测试** —— Rust 单元 + 集成、React 组件、关键流程 E2E
- **每完成一个 task** —— 立即追加 [`任务报告.md`](./docs/任务报告.md) → 检查 context → > 40% 执行 `/compact`
- **UI/UX** —— 先 `/apple-design` 再 `/frontend-design`
- **代码规范** —— Rust → `/rust-best-practices`；React → `/vercel-react-best-practices`；Next.js → `/next-best-practices`
- **样式** —— 仅 Tailwind utility + antd v6 `ConfigProvider` token；禁用 CSS Modules / styled-components / Sass / 全局 CSS（`globals.css` 入口除外）
- **提交** —— 每完成一个 task 立即 commit + push（用户长期授权，2026-04-28 起生效；commit message 含 task ID）

---

## 发布

版本号驱动的全自动流水线：

1. 把 `apps/desktop/src-tauri/tauri.conf.json` 的 `version` 改成目标值
2. 合到 `main`
3. GitHub Actions 自动：检测版本 → 打 tag `v<version>` → 三平台 matrix 构建 → 创建 Release

| `version` | Release 类型 | 产物                                              |
| --------- | ------------ | ------------------------------------------------- |
| `0.0.0`   | 跳过（占位） | —                                                 |
| `0.x.y`   | pre-release  | `*_aarch64.dmg` / `*_x64.dmg` / `*_x64-setup.exe` |
| `1.0.0+`  | 正式 release | 同上                                              |

### macOS 提示 "CCUse 已损坏"

由于当前 CI 暂未启用 Apple Developer 公证，从浏览器下载的 dmg 会被 Gatekeeper 误标记为"已损坏"。**这不是真的损坏**，确认 dmg 来自 [GitHub Release](https://github.com/colna/CCUse/releases) 后，把 `CCUse.app` 拖进 `/Applications` 并在终端执行一次：

```bash
sudo xattr -dr com.apple.quarantine /Applications/CCUse.app
```

之后即可正常启动。详细原理与说明见 [`docs/faq-troubleshooting`](./apps/website/content/docs/zh/faq-troubleshooting.mdx)。

---

## License

待定。
