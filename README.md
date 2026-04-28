# CCUse

> 本地 API 代理 + 多供应商无感切换 —— 一次配置，全局自动故障转移。

CCUse 是一款桌面应用，在本地启动统一的 HTTP API 代理服务，将多个 AI 服务供应商（Claude / OpenAI / Gemini / 中转商 / 自定义端点）抽象为单一接口。任何支持自定义 API 的客户端（Claude Desktop、Cursor、Continue 等）只需指向本地代理，即可获得：

- 单次配置 → 全局生效
- 后台健康检查 + 自动故障转移
- 多种切换策略（优先级 / 智能 / 负载均衡 / 最快响应 / 成本优先）
- 实时监控与统计
- 跨平台（macOS Apple Silicon / macOS Intel / Windows）

---

## 当前状态

|            |                                               |
| ---------- | --------------------------------------------- |
| 版本       | `0.4.0`（Phase 1.0.4 完成，UI + 监控 + i18n） |
| 路线图     | 详见 [`开发计划.md`](./docs/开发计划.md)      |
| 下一个版本 | `1.0.0`（Phase 1.0.5 GA 发布）                |

---

## 文档索引

| 文档                                                               | 内容                                                                        |
| ------------------------------------------------------------------ | --------------------------------------------------------------------------- |
| [`用户手册`](./docs/user-manual.md)                                | 配置、切换策略、监控、导入导出使用说明（中英双语）                          |
| [`FAQ`](./docs/FAQ.md)                                             | 常见问题与排错（端口冲突、WebView2、公证、签名等）                          |
| [`产品技术文档.md`](./docs/产品技术文档.md)                        | 产品定位、形态、功能、架构、技术选型、数据库设计、安全性                    |
| [`开发计划.md`](./docs/开发计划.md)                                | 5 个 Phase + 官网 3 个 Phase，所有 task 按 ID / 工时 / 依赖列出             |
| [`CLAUDE.md`](./CLAUDE.md)                                         | 项目工作规则（测试、任务报告、context、设计/代码 best-practices skill）     |
| [`任务报告.md`](./docs/任务报告.md)                                | append-only 任务执行记录                                                    |
| [`.github/workflows/release.yml`](./.github/workflows/release.yml) | 自动化发布：版本号驱动，三平台产物（aarch64.dmg / x64.dmg / x64-setup.exe） |

---

## 仓库结构（monorepo）

```
CCUse/
├── apps/
│   ├── desktop/          # Tauri 2 + React 18 + TS（当前唯一活跃包）
│   │   ├── src/          # React 前端
│   │   └── src-tauri/    # Rust 后端
│   └── website/          # Next.js 14 官网（Phase 1.0.W 启用）
├── packages/
│   └── ui/               # 跨端共享 shadcn 组件 + Tailwind preset（Phase 1.0.W 启用）
├── .github/workflows/
│   └── release.yml       # 自动 tag + 三平台构建 + GitHub Release
├── pnpm-workspace.yaml
└── package.json          # 根 scripts 转发器
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

# 2. 启动桌面端（dev 模式）
pnpm desktop:dev

# 3. 跑测试
pnpm desktop:test           # Vitest（React 端）
cd apps/desktop/src-tauri && cargo test    # cargo test（Rust 端）

# 4. 类型检查
pnpm desktop:typecheck
```

> Phase 1.0.1 → 1.0.4 已完成。本地代理 + 多供应商管理 + 自动切换 + 格式转换 + 监控面板 + 系统托盘 + i18n 均已就绪。详见 [`开发计划.md`](./docs/开发计划.md)。

---

## Phase 1.0.1 demo 验证

Phase 1.0.1 落地的是「本地代理骨架 + auth 中间件 + Provider trait + UI Shell」。真正的"客户端 → 本地代理 → 上游模型"端到端转发要等 T1.0.2.15（`SwitchEngine`）。在那之前，可以用脚本核对当前 wire 契约：

```bash
bash scripts/verify-phase-1-0-1.sh
```

脚本会：

1. cargo build & 启动一个 ephemeral-port proxy（`apps/desktop/src-tauri/examples/run_proxy.rs`）
2. 用 `curl` 跑 6 条契约：
   - `GET /healthz` 返回 `200 ok`（无需鉴权）
   - `GET /v1/models` 无 key → 401 unauthorized
   - `GET /v1/models` + `Authorization: Bearer sk-local-…` → 200
   - `POST /v1/chat/completions` 带 key → 503 + `OpenAI`-shaped `providers_not_configured` 错误体（stub）
   - `POST /v1/messages` 带 `x-api-key` → 同样 503 stub（Anthropic 风格头部接受）
   - 来自 `https://evil.example.com` 的 CORS 预检 → 无 `Access-Control-Allow-Origin` 头
3. 任一失败即非零退出

> **关于 GIF**：开发计划中 T1.0.1.27 原计划录制 "Cursor 配置本地接口" 的 GIF。由于 chat 路由仍是 503 stub，端到端 Cursor demo 必须等 T1.0.2.15 接入 SwitchEngine 后再录，已登记到 `docs/任务报告.md` 的 follow-up。

---

## 开发流程

> 任何在本仓库内的工作，都必须先读 [`CLAUDE.md`](./CLAUDE.md) —— 包含强制的测试要求、任务报告格式、context 管理阈值、设计 / 代码 best-practices skill 调用顺序。

要点速览：

- **每个功能必有测试**（Rust 单元 + 集成、React 组件、关键流程 E2E）
- **每完成一个 task** → 立即追加 [`任务报告.md`](./docs/任务报告.md) → 检查 context → > 40% 执行 `/compact`
- **UI/UX**：先 `/apple-design` 再 `/frontend-design`
- **代码规范**：Rust → `/rust-best-practices`；React → `/vercel-react-best-practices`；Next.js → `/next-best-practices`
- **样式仅 Tailwind**（`globals.css` 入口外禁止 `*.css`）
- **每完成一个 task 立即 commit + push**（用户长期授权，2026-04-28 起生效；commit message 含 task ID）

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

---

## License

待定。
