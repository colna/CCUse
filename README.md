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

| | |
|--|--|
| 版本 | `0.0.0`（脚手架，未编码完成） |
| 路线图 | 详见 [`开发计划.md`](./开发计划.md) |
| 下一个版本 | `0.1.0`（Phase 1.0.1 完成时打 tag，pre-release） |
| GA 目标 | `1.0.0`（Phase 1.0.5 完成时） |

---

## 文档索引

| 文档 | 内容 |
|------|------|
| [`产品技术文档.md`](./产品技术文档.md) | 产品定位、形态、功能、架构、技术选型、数据库设计、安全性 |
| [`开发计划.md`](./开发计划.md) | 5 个 Phase + 官网 3 个 Phase，所有 task 按 ID / 工时 / 依赖列出 |
| [`CLAUDE.md`](./CLAUDE.md) | 项目工作规则（测试、任务报告、context、设计/代码 best-practices skill） |
| [`任务报告.md`](./任务报告.md) | append-only 任务执行记录 |
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

> 当前 `0.0.0` 阶段仅有最小脚手架页面 + 一个 Rust smoke test。功能将随 Phase 1.0.1 → 1.0.5 任务陆续接入，详见 [`开发计划.md`](./开发计划.md)。

---

## 开发流程

> 任何在本仓库内的工作，都必须先读 [`CLAUDE.md`](./CLAUDE.md) —— 包含强制的测试要求、任务报告格式、context 管理阈值、设计 / 代码 best-practices skill 调用顺序。

要点速览：
- **每个功能必有测试**（Rust 单元 + 集成、React 组件、关键流程 E2E）
- **每完成一个 task** → 立即追加 [`任务报告.md`](./任务报告.md) → 检查 context → > 40% 执行 `/compact`
- **UI/UX**：先 `/apple-design` 再 `/frontend-design`
- **代码规范**：Rust → `/rust-best-practices`；React → `/vercel-react-best-practices`；Next.js → `/next-best-practices`
- **样式仅 Tailwind**（`globals.css` 入口外禁止 `*.css`）
- **不自动 commit / push**，等明确指令

---

## 发布

版本号驱动的全自动流水线：

1. 把 `apps/desktop/src-tauri/tauri.conf.json` 的 `version` 改成目标值
2. 合到 `main`
3. GitHub Actions 自动：检测版本 → 打 tag `v<version>` → 三平台 matrix 构建 → 创建 Release

| `version` | Release 类型 | 产物 |
|-----------|--------------|------|
| `0.0.0` | 跳过（占位） | — |
| `0.x.y` | pre-release | `*_aarch64.dmg` / `*_x64.dmg` / `*_x64-setup.exe` |
| `1.0.0+` | 正式 release | 同上 |

---

## License

待定。
