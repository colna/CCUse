# CCUse User Manual / 用户手册

## Table of Contents

- [Getting Started / 快速开始](#getting-started)
- [Providers / 供应商配置](#providers)
- [Switch Strategy / 切换策略](#switch-strategy)
- [Monitoring / 监控面板](#monitoring)
- [Config Export & Import / 配置导入导出](#config-export)
- [System Tray / 系统托盘](#system-tray)
- [Language / 语言切换](#language)

---

## Getting Started

### EN

1. **Install** — Download the installer for your platform from [GitHub Releases](https://github.com/colna/CCUse/releases):
   - macOS Apple Silicon: `CCUse_x.y.z_aarch64.dmg`
   - macOS Intel: `CCUse_x.y.z_x64.dmg`
   - Windows: `CCUse_x.y.z_x64-setup.exe`
2. **Launch** — Open CCUse. The local API proxy starts automatically on a random available port.
3. **Add a provider** — Go to the Providers page and add at least one AI provider (OpenAI, Anthropic, Gemini, relay, or custom).
4. **Configure your client** — Point your AI client (Cursor, Claude Desktop, Continue, etc.) to the local proxy URL shown on the Dashboard. Use the generated API key for authentication.
5. **Done** — All requests now route through CCUse with automatic failover.

### ZH

1. **安装** — 从 [GitHub Releases](https://github.com/colna/CCUse/releases) 下载对应平台安装包。
2. **启动** — 打开 CCUse，本地 API 代理会自动在可用端口上启动。
3. **添加供应商** — 进入"供应商"页面，添加至少一个 AI 供应商（OpenAI、Anthropic、Gemini、中转商或自定义端点）。
4. **配置客户端** — 将 AI 客户端（Cursor、Claude Desktop、Continue 等）的 API 地址指向仪表盘显示的本地代理 URL，使用生成的 API Key 鉴权。
5. **完成** — 所有请求现在经由 CCUse 路由，自动故障转移。

---

## Providers

### EN

CCUse supports 5 provider types:

| Type          | Description                                          | Default Base URL                            |
| ------------- | ---------------------------------------------------- | ------------------------------------------- |
| **OpenAI**    | OpenAI API (`/v1/chat/completions`)                  | `https://api.openai.com`                    |
| **Anthropic** | Anthropic API (`/v1/messages`)                       | `https://api.anthropic.com`                 |
| **Gemini**    | Google Gemini (`/v1beta/models/.../generateContent`) | `https://generativelanguage.googleapis.com` |
| **Relay**     | Relay/proxy endpoint (OpenRouter, One API, etc.)     | —                                           |
| **Custom**    | Any OpenAI-compatible endpoint                       | —                                           |

**Adding a provider:**

1. Click "Add Provider" on the Providers page.
2. Select the provider type.
3. Enter name, API key, and base URL (pre-filled for standard providers).
4. Optionally set priority (lower = higher priority), monthly quota, rate limit, and cost per 1K tokens.
5. Click "Test Connection" to verify, then "Add Provider".

**Managing providers:**

- **Drag to reorder** — Drag the grip handle to change priority order.
- **Edit** — Click the pencil icon to inline-edit name, URL, priority, or enabled state.
- **Enable/Disable** — Toggle the checkbox to temporarily disable a provider without deleting it.
- **Delete** — Click the trash icon (requires confirmation).

### ZH

CCUse 支持 5 种供应商类型：

| 类型          | 说明                                    | 默认 Base URL                               |
| ------------- | --------------------------------------- | ------------------------------------------- |
| **OpenAI**    | OpenAI API                              | `https://api.openai.com`                    |
| **Anthropic** | Anthropic API                           | `https://api.anthropic.com`                 |
| **Gemini**    | Google Gemini                           | `https://generativelanguage.googleapis.com` |
| **Relay**     | 中转/代理端点（OpenRouter、One API 等） | —                                           |
| **Custom**    | 任意 OpenAI 兼容端点                    | —                                           |

**添加供应商：**

1. 在"供应商"页面点击"添加供应商"。
2. 选择供应商类型。
3. 填写名称、API Key、Base URL（标准供应商会预填）。
4. 可选设置优先级（数字越小优先级越高）、月配额、速率限制、每千 token 成本。
5. 点击"测试连接"验证后，点击"添加"。

**管理供应商：**

- **拖拽排序** — 拖动手柄调整优先级。
- **编辑** — 点击铅笔图标内联编辑。
- **启用/禁用** — 切换复选框临时禁用。
- **删除** — 点击垃圾桶图标（需确认）。

---

## Switch Strategy

### EN

CCUse offers 5 switching strategies:

| Strategy         | How it works                                                                        |
| ---------------- | ----------------------------------------------------------------------------------- |
| **Priority**     | Routes to the highest-priority enabled provider. Falls back to the next on failure. |
| **Smart**        | Combines health score, latency, and cost to pick the optimal provider.              |
| **Load Balance** | Distributes requests across all healthy providers.                                  |
| **Fastest**      | Always picks the provider with the lowest recent latency.                           |
| **Cost**         | Picks the cheapest provider that is currently healthy.                              |

Go to the Strategy page to select and configure. Advanced parameters (health weight, latency weight, cost weight, failure threshold, cooldown) are available for the Smart strategy.

### ZH

CCUse 提供 5 种切换策略：

| 策略         | 工作方式                                       |
| ------------ | ---------------------------------------------- |
| **优先级**   | 路由到优先级最高的可用供应商，故障时依次降级。 |
| **智能**     | 综合健康度、延迟、成本选择最优供应商。         |
| **负载均衡** | 在所有健康供应商间分发请求。                   |
| **最快响应** | 总是选择最近延迟最低的供应商。                 |
| **成本优先** | 选择当前健康的最便宜供应商。                   |

在"策略"页面选择和配置。智能策略可调整高级参数（健康权重、延迟权重、成本权重、故障阈值、冷却时间）。

---

## Monitoring

### EN

The Dashboard provides real-time monitoring:

- **Status Cards** — Active providers, total requests, success rate, average latency.
- **Success Rate Chart** — 24-hour success rate trend (5-minute buckets).
- **Latency Chart** — Average and P95 latency over 24 hours.
- **Cost Chart** — Cost distribution by provider (pie chart).
- **Switch Timeline** — Recent provider switch events with reasons.

All charts auto-refresh every 30 seconds. Health status refreshes every 5 seconds.

### ZH

仪表盘提供实时监控：

- **状态卡片** — 活跃供应商数、总请求数、成功率、平均延迟。
- **成功率图表** — 24 小时成功率趋势（5 分钟粒度）。
- **延迟图表** — 24 小时平均 + P95 延迟。
- **成本分布** — 各供应商成本占比（饼图）。
- **切换时间线** — 最近的供应商切换事件及原因。

所有图表每 30 秒自动刷新，健康状态每 5 秒刷新。

---

## Config Export

### EN

Export and import your CCUse configuration (providers, strategy, model mappings) as encrypted files.

**Export:**

1. Go to Settings > Config Export.
2. Click "Export Configuration".
3. Enter a password to encrypt the file.
4. Save the `.ccuse` file.

**Import:**

1. Click "Import Configuration".
2. Select a `.ccuse` file.
3. Enter the password used during export.
4. Existing configuration will be replaced.

**Template Presets:** Quick-start templates for Claude, OpenAI, and Gemini configurations.

Encryption: scrypt KDF + AES-256-GCM. The file cannot be read without the password.

### ZH

导出和导入 CCUse 配置（供应商、策略、模型映射）为加密文件。

**导出：**

1. 进入设置 > 配置导出。
2. 点击"导出配置"。
3. 输入口令加密文件。
4. 保存 `.ccuse` 文件。

**导入：**

1. 点击"导入配置"。
2. 选择 `.ccuse` 文件。
3. 输入导出时使用的口令。
4. 现有配置将被替换。

加密方式：scrypt KDF + AES-256-GCM，无口令无法读取。

---

## System Tray

### EN

CCUse runs in the system tray. Closing the main window hides it to the tray instead of quitting.

Tray menu actions:

- **Show Window** — Bring the main window back.
- **Copy API Key** — Copy the local proxy API key to clipboard.
- **Restart Proxy** — Restart the local proxy server.
- **Quit** — Fully exit CCUse.

### ZH

CCUse 在系统托盘运行。关闭主窗口会最小化到托盘而非退出。

托盘菜单：

- **显示窗口** — 恢复主窗口。
- **复制 API Key** — 复制本地代理 API Key 到剪贴板。
- **重启代理** — 重启本地代理服务。
- **退出** — 完全退出 CCUse。

---

## Language

### EN

CCUse supports English and Chinese. Go to Settings to switch:

- **Follow System** — Automatically use your OS language.
- **English** — Force English.
- **Chinese** — Force Chinese.

The preference is persisted across sessions.

### ZH

CCUse 支持中英双语。在设置中切换：

- **跟随系统** — 自动使用操作系统语言。
- **English** — 强制英文。
- **中文** — 强制中文。

语言偏好会跨会话保存。
