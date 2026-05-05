# CCUse Codex 工作规则

> 本文件是 Codex 在本仓库工作的项目级规则，由 `CLAUDE.md` 转写而来。
> 每次新会话开始后必须先读取本文件，并按本文执行。
> 如本文与系统 / 开发者 / 用户的更高优先级指令冲突，遵循更高优先级指令；在仓库级规则中，本文优先于通用习惯。

---

## 0. 项目背景

- **产品定位**：本地 API 代理 + 多供应商无感切换桌面应用
- **目标平台**：macOS（Apple Silicon / Intel）+ Windows
- **产品形态**：桌面应用（Tauri 2 + Rust + React）+ 官网（Next.js 14）
- **关键文档**：
  - `docs/产品技术文档.md`：架构、技术选型、产品功能
  - `docs/开发计划.md`：分阶段任务清单（含 task ID、依赖、工时）
  - `docs/任务报告.md`：append-only 任务执行记录
  - `docs/无感切换方案-API代理模式.md` / `docs/自动切换供应商应用设计方案.md`：归档设计草案
  - `.github/workflows/release.yml`：自动化发布流水线

---

## 1. 会话开始检查清单

开始任何实质工作前，先完成以下检查：

- 已读取 `AGENTS.md`。
- 已按当前任务范围读取 `docs/产品技术文档.md` 与 `docs/开发计划.md` 的相关章节。
- 已读取 `docs/任务报告.md` 最近若干条，了解已完成工作和遗留事项。
- 已确认当前 task ID；如果用户未指明且任务属于开发计划，先从 `docs/开发计划.md` 定位或向用户确认。
- 已检查 `git status --short`，识别用户已有改动；不得回滚与当前任务无关的改动。
- 涉及 UI / UX / 视觉 / 交互时，先使用 `apple-design`，再使用 `frontend-design`。
- 涉及代码时，先使用对应 best-practices skill，见 §6。

---

## 2. 任务执行流程（强制）

对 `docs/开发计划.md` 中编号为 `T1.0.x.xx` / `TW.x.xx` 的 task，每完成 1 个 task 必须按顺序执行：

1. **写测试**：按 §3 的测试规则补齐自动化测试。
2. **跑验证**：相关测试必须通过；高风险改动要扩大验证范围。
3. **更新任务报告**：按 §4 立即追加到 `docs/任务报告.md`。
4. **检查 context 用量**：按 §5 执行。
5. **必要时压缩 / 交接 context**：先写 scratchpad，再压缩或请求用户开启新会话。
6. **提交代码并 push**：按 §8 执行；一个 task 对应一个或多个原子 commit。

“完成”的定义：代码合入 + 测试通过 + 任务报告已更新 + git commit + git push。五者缺一不算完成。

临时性维护任务（例如只新增本文件）如果不对应 `docs/开发计划.md` 中的 task ID，完成时要在最终回复中说明未登记任务报告和未提交的原因，除非用户明确要求提交。

---

## 3. 测试规则（强制）

### 3.1 每个功能都必须有测试

凡开发计划中的功能 task，完成时必须包含对应自动化测试。无测试 = 任务未完成。

### 3.2 测试层级要求

| 任务类型                                                             | 必须的测试层级                                  | 工具                                  |
| -------------------------------------------------------------------- | ----------------------------------------------- | ------------------------------------- |
| Rust 业务逻辑（Provider / SwitchEngine / Converter / HealthChecker） | 单元测试 + 至少 1 条集成测试                    | `cargo test` + `wiremock`             |
| Tauri command                                                        | 单元测试（mock state）                          | `cargo test`                          |
| React 组件（UI 元素 / 表单 / 卡片）                                  | 组件测试（render + interaction）                | Vitest + Testing Library              |
| 关键用户流程（添加供应商、自动切换、下载安装包）                     | E2E 测试                                        | Playwright（桌面端用 `tauri-driver`） |
| 格式转换器（OpenAI / Anthropic / Gemini）                            | 单元测试覆盖率 >= 90%                           | `cargo test` + `cargo-llvm-cov`       |
| 官网页面                                                             | 至少 1 条 Playwright 冒烟 + Lighthouse CI >= 90 | Playwright + Lighthouse CI            |

### 3.3 测试位置

```text
apps/desktop/src-tauri/src/**/*.rs       # 同文件 #[cfg(test)] mod tests
apps/desktop/src-tauri/tests/**.rs       # 集成测试
apps/desktop/src/**/__tests__/*.test.tsx # React 组件测试
apps/desktop/e2e/**.spec.ts              # 桌面 E2E
apps/website/__tests__/**.test.tsx       # Next.js 组件测试
apps/website/e2e/**.spec.ts              # 官网 E2E
```

### 3.4 测试基线

- `cargo test` / `pnpm test` 必须全绿才能继续下一个 task。
- 核心模块（converter、switch_engine、health_checker、auth）覆盖率 >= 85%。
- E2E 不允许 flaky；连续 3 次重跑仍失败必须修复。
- mock 优先（`wiremock` / MSW），禁止用真实供应商 API Key 跑 CI。

---

## 4. 任务报告文件（强制）

### 4.1 路径

`docs/任务报告.md`

### 4.2 何时更新

每个开发计划 task 完成时立即追加一条记录，不得批量补登。

### 4.3 记录格式（Append-only）

在文件末尾追加：

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
- **Context 检查**: x%（>40% 已执行压缩 / 交接）
- **风险 / 后续 todo**: 无 / xxx
- **备注**: xxx
```

### 4.4 报告文件结构

- 第一行必须是 `# CCUse 任务执行报告`。
- 之后按时间顺序追加；禁止修改历史记录。
- 如历史记录确需说明，只能追加 `## 修订` 子段。
- 每周五在文件末尾追加 `## 周回顾 [YYYY-WW]`，统计完成 task 数、累计工时、阻塞项。

---

## 5. Context 管理（Codex 适配）

### 5.1 检查时机

每完成 1 个开发计划 task 后立即检查 context 状态，即使 task 很小也要检查。

### 5.2 阈值规则

| context 占比 | 动作                                                   |
| ------------ | ------------------------------------------------------ |
| <= 40%       | 继续下一个 task                                        |
| > 40%        | 先写任务报告和 scratchpad，再执行可用的压缩 / 交接流程 |
| >= 80%       | 强制压缩 / 交接；如果压缩后仍明显偏高，停下并告知用户  |

### 5.3 Codex 中如何执行

- 如果当前环境提供明确 token / context 百分比，按百分比执行。
- 如果没有明确百分比，用会话长度、文件读取量、变更复杂度保守判断。
- 如果可用 `/compact` 或等价压缩功能，先完成 §5.4，再触发压缩。
- 如果没有手动压缩能力，先写 `.claude/scratchpad.md`，并在最终回复中说明当前进度、关键文件、未完成事项。

### 5.4 压缩 / 交接前必须做

- 把当前 task 的报告写入 `docs/任务报告.md`。
- 把进行中的 task ID、文件路径、待办点写入 `.claude/scratchpad.md`（如不存在则创建）。
- 再执行压缩、交接或请求用户开启新会话。

---

## 6. Skill 调用规则（强制）

### 6.1 UI / UX / 视觉 / 交互

凡涉及桌面端、官网、文档示意图、组件样式、图标、动效、空数据态、通知、托盘菜单等 UI/UX 工作，必须按顺序使用：

1. `apple-design`
2. `frontend-design`

如果 skill 不可用，停下并告知用户确认 skill 路径；不得用记忆或猜测内容代替 skill 输出。

### 6.2 代码 best-practices

提交任何代码前，根据语言 / 框架使用对应 skill：

| 语言 / 框架                     | 必须使用的 skill              | 触发场景                        |
| ------------------------------- | ----------------------------- | ------------------------------- |
| Rust（含 Tauri / axum / tokio） | `rust-best-practices`         | 任何 `.rs` 文件新增或修改       |
| React（桌面端 + 官网）          | `vercel-react-best-practices` | 任何 `.tsx` / `.ts`（含 hooks） |
| Next.js（官网）                 | `next-best-practices`         | `apps/website/**` 内任何文件    |

调用顺序：

1. 计划写代码前先读取对应 skill。
2. 将 skill 要点纳入实现思路。
3. 写代码 / 改代码。
4. 写测试。
5. 自查并修正违反规范的实现。

如果必需的 skill 不可用，先告知用户确认路径或安装方式，不要凭记忆替代。

---

## 7. 代码与工程基线

- **语言**：默认中文回复；技术术语和标识符保持原文。
- **包管理器**：使用 `pnpm`，遵循根 `package.json` scripts。
- **搜索**：优先使用 `rg` / `rg --files`。
- **编辑**：手工文件修改使用 `apply_patch`；不要用脚本或 shell 重写无关内容。
- **用户改动**：工作区可能已有用户改动；不得回滚、覆盖、格式化与当前任务无关的文件。
- **注释**：默认不写；只在“为什么这样做”不显然时加一行。
- **样式**：仅 Tailwind utility；除既有入口外，禁止 CSS Modules / styled-components / Sass / 全局 CSS。
- **产物**：发布产物严格 3 个：`*_aarch64.dmg` / `*_x64.dmg` / `*_x64-setup.exe`；禁用其他 bundle target。
- **版本号**：`0.x.y` 为 pre-release；`1.0.0+` 为正式 release；详见 `docs/开发计划.md`。
- **真实密钥**：禁止在测试、日志、提交中使用或泄露真实供应商 API Key。
- **绕过 hook**：严禁 `--no-verify`、`--no-gpg-sign` 等绕过手段。
- **销毁性操作**：删除、强推、重置、清理大范围文件等必须获得用户明确授权。

---

## 8. Git 规则

- 开始前用 `git status --short` 查看工作区。
- 每完成一个开发计划 task，立即 commit + push。
- 使用 Conventional Commits。
- commit message 必须包含对应 task ID，例如：

```text
feat(desktop): add provider health check T1.0.1.07
```

- 一个 task 可拆多个原子 commit，但每个 commit 都要能独立说明变更。
- 不要把无关用户改动混入自己的 commit。
- 不要使用 `git reset --hard`、`git checkout -- <path>`、强推等销毁性命令，除非用户明确要求。

---

## 9. 常用命令

```bash
pnpm install
pnpm desktop:dev
pnpm desktop:build
pnpm desktop:test
pnpm desktop:typecheck
pnpm website:dev
pnpm website:build
pnpm website:test
pnpm website:typecheck
pnpm lint
pnpm typecheck
pnpm format:check
cd apps/desktop/src-tauri && cargo test
```

根据变更范围选择最小但充分的验证命令；共享模块、跨端契约或发布流程变更需要扩大验证范围。

---

## 10. 违规处理

如果发现已经违反本文规则（例如忘记写测试、忘记更新任务报告、忘记检查 context、误碰用户改动），立即停下当前工作，先补做缺漏步骤，再继续。

不要把必须步骤留到“下次再补”。
