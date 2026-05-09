# CCUse 项目工作规则

> 本文档对所有未来在本仓库内的会话生效。每次新会话开启后，必须先读取本文件并严格遵循。
> 与全局 `~/.claude/CLAUDE.md` 同时生效；本项目规则**优先级更高**。

---

## 0. 项目背景

- **产品定位**：本地 API 代理 + 多供应商无感切换桌面应用
- **目标平台**：macOS（Apple Silicon / Intel）+ Windows
- **形态**：桌面应用（Tauri 2 + Rust + React）+ 官网（Next.js 14）
- **关键文档**（必读，全部位于 `docs/`）：
  - `docs/产品技术文档.md` — 架构、技术选型、产品功能
  - `docs/开发计划.md` — 分阶段任务清单（含 task ID、依赖、工时）
  - `docs/任务报告.md` — append-only 任务执行记录
  - `docs/无感切换方案-API代理模式.md` / `docs/自动切换供应商应用设计方案.md` — 整合前的设计草案，归档保留
  - `.github/workflows/release.yml` — 自动化发布流水线
- **当前版本**：`0.0.0`（仅文档，未编码）

---

## 1. 任务执行流程（强制）

每完成 1 个 task（task ID 见 `docs/开发计划.md`），按以下顺序执行，**不得跳步**：

1. **写测试** —— 见 §2
2. **更新任务报告** —— 见 §3
3. **检查 context 用量** —— 见 §4
4. **必要时压缩 context** —— 见 §4
5. **提交代码** —— 见 §7（本步骤为用户长期授权，无需每次再问）

> "完成"的定义：代码合入 + 测试通过 + 任务报告已更新 + git commit & push。**五者**缺一不算完成。

---

## 2. 测试规则（强制，不可绕过）

### 2.1 每个功能都必须有测试

凡 `docs/开发计划.md` 中编号为 `T1.0.x.xx` / `TW.x.xx` 的任务，**完成时必须包含**对应的自动化测试。无测试 = 任务未完成。

### 2.2 测试层级要求

| 任务类型                                                             | 必须的测试层级                                 | 工具                                  |
| -------------------------------------------------------------------- | ---------------------------------------------- | ------------------------------------- |
| Rust 业务逻辑（Provider / SwitchEngine / Converter / HealthChecker） | 单元测试 + 至少 1 条集成测试                   | `cargo test` + `wiremock`             |
| Tauri command                                                        | 单元测试（mock state）                         | `cargo test`                          |
| React 组件（UI 元素 / 表单 / 卡片）                                  | 组件测试（render + interaction）               | Vitest + Testing Library              |
| 关键用户流程（添加供应商、自动切换、下载安装包）                     | E2E 测试                                       | Playwright（桌面端用 `tauri-driver`） |
| 格式转换器（OpenAI / Anthropic / Gemini）                            | 单元测试覆盖率 ≥ 90%                           | `cargo test` + `cargo-llvm-cov`       |
| 官网页面                                                             | 至少 1 条 Playwright 冒烟 + Lighthouse CI ≥ 90 | Playwright + Lighthouse CI            |

### 2.3 测试写在哪里

```
apps/desktop/src-tauri/src/**/*.rs       # 同文件 #[cfg(test)] mod tests
apps/desktop/src-tauri/tests/**.rs       # 集成测试
apps/desktop/src/**/__tests__/*.test.tsx # React 组件测试
apps/desktop/e2e/**.spec.ts              # 桌面 E2E
apps/website/__tests__/**.test.tsx       # Next.js 组件测试
apps/website/e2e/**.spec.ts              # 官网 E2E
```

### 2.4 测试基线

- `cargo test` / `pnpm test` 必须全绿才能继续下一个 task
- 核心模块（converter、switch_engine、health_checker、auth）覆盖率 **≥ 85%**
- E2E 不允许 flaky；连续 3 次重跑还失败必须修复
- mock 优先（`wiremock` / MSW），**禁止**用真实的供应商 API Key 跑 CI

---

## 3. 任务报告文件（强制）

### 3.1 路径

`/Users/colna/WORK/CCUse/docs/任务报告.md`

### 3.2 何时更新

**每个 task 完成时立即追加一条记录**，不得批量补登。

### 3.3 记录格式（Append-only）

每条记录以下面的模板追加到文件末尾：

```markdown
## [YYYY-MM-DD HH:MM] T<task-id> <task 标题>

- **Phase**: 1.0.x（或 1.0.W.x）
- **状态**: ✅ 完成 / ⚠️ 部分完成 / ❌ 失败
- **预估工时**: 0.5d
- **实际工时**: 0.5d
- **变更文件**:
  - `path/to/file.rs`（新增 / 修改 / 删除）
- **测试**:
  - 单元测试：`cargo test --package xxx -- xxx::tests` 通过 N 条
  - 集成测试：xxx
  - E2E：xxx
  - 覆盖率：xx%
- **遵循的 best-practices skill**: `/rust-best-practices` / `/vercel-react-best-practices` / `/next-best-practices`
- **设计 skill 调用**（如涉及 UI/UX）: `/apple-design` + `/frontend-design`
- **Context 检查**: x% （>40% 已执行 /compact）
- **风险 / 后续 todo**: 无 / xxx
- **备注**: xxx
```

### 3.4 报告文件结构

- 第一行 `# CCUse 任务执行报告`
- 之后按时间顺序追加；**禁止**修改历史记录（只允许追加 `## 修订` 子段说明改动）
- 每周五在文件末尾追加 `## 周回顾 [YYYY-WW]`，统计：完成 task 数、累计工时、阻塞项

---

## 4. Context 管理（强制）

### 4.1 检查时机

**每完成 1 个 task** 后立即检查（即使 task 很小也要检查）。检查方式：调用 `/context` 或读取系统返回的 token 用量。

### 4.2 阈值规则

| context 占比 | 动作                                                 |
| ------------ | ---------------------------------------------------- |
| ≤ 40%        | 继续下一个 task                                      |
| > 40%        | **立即执行 `/compact`**，压缩完成后再开始下一个 task |
| ≥ 80%        | 强制 `/compact`；如压缩后仍 ≥ 60%，停下并告知用户    |

### 4.3 压缩前必须做

- 把当前 task 的报告写入 `docs/任务报告.md`（避免压缩丢上下文）
- 把进行中的 task ID、文件路径、待办点写入 `.claude/scratchpad.md`（如不存在则创建）
- 然后再 `/compact`

---

## 5. 设计规则（强制）

凡涉及 **UI / UX / 视觉 / 交互**（无论桌面端、官网，或文档示意图），必须按顺序调用以下 skill 后才动手：

1. **先 `/apple-design`** —— 获取 Apple HIG 风格的设计指引（间距、字号、动画、深浅色、控件层次）
2. **再 `/frontend-design`** —— 获取前端落地层面的实现规范（组件拆分、状态、可访问性、响应式）

> 仅完全不涉及 UI 的纯逻辑 task（Rust 后端、CI、测试基础设施）可以跳过。
>
> 注意：上述两个 skill 当前未出现在会话默认 skill 清单中，首次调用如提示找不到，应在调用方告知用户确认 skill 路径，不得用记忆 / 猜测内容代替 skill 输出。

涉及的具体场景至少包含：

- shadcn/ui 组件二次封装、Tailwind class 取值
- Recharts 图表配色与图例
- 桌面端托盘菜单、桌面通知文案与样式
- 官网 Hero、Features、Docs、Download 页面
- 任何新增图标、动效、空数据态

---

## 6. 代码规则（按语言/框架强制）

提交任何代码前，必须根据语言/框架先调用对应 best-practices skill 获取最新规范，然后按规范写代码 / review 代码：

| 语言 / 框架                     | 必须调用的 skill                   | 触发场景                      |
| ------------------------------- | ---------------------------------- | ----------------------------- |
| Rust（含 Tauri / axum / tokio） | **`/rust-best-practices`**         | 任何 `.rs` 文件新增或修改     |
| React（桌面端 + 官网）          | **`/vercel-react-best-practices`** | 任何 `.tsx` `.ts`（含 hooks） |
| Next.js（官网）                 | **`/next-best-practices`**         | `apps/website/**` 内任何文件  |

> 注：`/next-best-practices` 当前未出现在会话默认 skill 清单中，首次调用如提示找不到，应告知用户确认 skill 路径或暂以 `/vercel-react-best-practices` 中的 Next 部分代替，不得用记忆 / 猜测内容代替 skill 输出。

调用顺序：

1. 计划要写的代码 → 先 `Skill` 调用对应 skill
2. 把 skill 返回的规范要点纳入实现思路
3. 写代码 / 改代码
4. 写测试（§2）
5. 自查是否违反规范；违反则改回符合的写法

---

## 7. 风格 / 工程基线

继承全局 `~/.claude/CLAUDE.md` 的所有规则，并在此基础上叠加：

| 维度       | 规则                                                                                                                                                                                                                             |
| ---------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 语言       | 中文回复；技术术语 / 标识符保持原文                                                                                                                                                                                              |
| 注释       | 默认不写；只在 _为什么_ 不显然时加一行                                                                                                                                                                                           |
| 提交       | Conventional Commits；**每完成一个 task 立即 `git commit + git push`**（用户长期授权，2026-04-28 起生效）。一个 task 一个或多个原子 commit，commit message 含对应 task ID（如 `T1.0.1.07`）                                      |
| 样式       | 仅 Tailwind utility 与 antd v6 的 `@ant-design/cssinjs` token 体系；禁用 CSS Modules / styled-components / Sass / 全局 CSS（`globals.css` 入口除外）。组件视觉走 antd `ConfigProvider` Token，布局/间距/色彩 token 仍用 Tailwind |
| 产物       | 严格 3 个：`*_aarch64.dmg` / `*_x64.dmg` / `*_x64-setup.exe`；其余 bundle target 禁用                                                                                                                                            |
| 版本号     | `0.x.y` = pre-release；`1.0.0+` = 正式 release；详见 `docs/开发计划.md` §一                                                                                                                                                      |
| 销毁性操作 | 删除 / 强推 / 重置等需用户明确授权                                                                                                                                                                                               |
| 跳过 hook  | 严禁 `--no-verify`、`--no-gpg-sign` 等绕过手段                                                                                                                                                                                   |

---

## 8. 会话开始检查清单

每次新会话首条工作消息前，必须确认（在思考中或简短告知用户）：

- [ ] 已读取本文件（`CCUse/CLAUDE.md`）
- [ ] 已读取 `docs/产品技术文档.md` 与 `docs/开发计划.md` 的相关章节（按当前 task 范围）
- [ ] 已读取 `docs/任务报告.md` 的最近若干条，了解上下文
- [ ] 知道当前要做的 task ID（若用户未指明，先 `TaskList` 或问用户）
- [ ] 涉及 UI → 先调 `/apple-design` + `/frontend-design`
- [ ] 涉及代码 → 先调对应语言的 best-practices skill
- [ ] 任务做完 → 更新 `docs/任务报告.md` → 检查 context → 必要时 `/compact`

---

## 9. 违规处理

如果发现自己已经违反上述规则（例如忘了写测试、忘了更新任务报告、忘了检查 context），**立即停下当前工作**，先补做缺漏的步骤，再继续。
不要试图"下次再补"。
