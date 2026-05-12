import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/**
 * 这个模块是 React 端与 Tauri 后端的唯一边界：所有 `invoke`/`listen`
 * 调用都收拢在这里，组件不直接接触 `@tauri-apps/api`。
 *
 * 类型与字段名严格镜像 Rust 端（`apps/desktop/src-tauri/src/...`），
 * 用下划线命名是因为后端用 serde 默认序列化，没必要在中间再加一层
 * camelCase 转换。
 */

// ─── Local API 代理（本地 8787 端口的统一入口）─────────────────────────

/** 后端 `proxy::runtime::LocalApiEndpointConfig` 的镜像。 */
export interface LocalApiEndpointConfig {
  base_url: string;
  api_key: string;
}

/** 后端 `proxy::runtime::LocalApiConfig` 的镜像。 */
export interface LocalApiConfig {
  /** 旧版根 URL；新 UI 使用协议分组里的 URL。 */
  base_url: string;
  /** 旧版 Key；等同于 OpenAI-compatible key。 */
  api_key: string;
  openai: LocalApiEndpointConfig;
  anthropic: LocalApiEndpointConfig;
}

export async function getLocalApiConfig(): Promise<LocalApiConfig> {
  return invoke<LocalApiConfig>("get_local_api_config");
}

export async function regenerateApiKey(): Promise<LocalApiConfig> {
  return invoke<LocalApiConfig>("regenerate_api_key");
}

export async function restartProxy(): Promise<LocalApiConfig> {
  return invoke<LocalApiConfig>("restart_proxy");
}

/**
 * 后端 key 轮换 / 端口变更后会发这个事件；组件用它即时刷新展示。
 * 事件名固定，两端测试都钉住这个常量。
 */
export const EVENT_LOCAL_API_CONFIG_CHANGED = "local_api_config_changed";

/** 订阅本地代理配置变更；返回值需要在 effect cleanup 中调用以取消订阅。 */
export async function onLocalApiConfigChanged(
  callback: (config: LocalApiConfig) => void,
): Promise<UnlistenFn> {
  return listen<LocalApiConfig>(EVENT_LOCAL_API_CONFIG_CHANGED, (event) => {
    callback(event.payload);
  });
}

// ─── Provider 增删改查 ─────────────────────────────────────────────────

/** 用户在表单里填写的供应商输入，对应 Rust `ProviderInput`。 */
export interface ProviderInput {
  name: string;
  kind: "openai" | "anthropic" | "gemini" | "relay" | "custom";
  base_url: string;
  api_key: string;
  priority: number;
  enabled: boolean;
  monthly_quota?: number | null;
  rate_limit_rpm?: number | null;
  cost_per_1k_tokens?: number | null;
}

/** 持久化后的供应商；`api_key` 出于安全原因不回传，所以这里不存在。 */
export interface Provider {
  id: string;
  name: string;
  kind: ProviderInput["kind"];
  base_url: string;
  priority: number;
  enabled: boolean;
  monthly_quota?: number | null;
  rate_limit_rpm?: number | null;
  cost_per_1k_tokens?: number | null;
  created_at: string;
  updated_at: string;
}

export async function listProviders(): Promise<Provider[]> {
  return invoke<Provider[]>("list_providers");
}

export async function addProvider(input: ProviderInput): Promise<Provider> {
  return invoke<Provider>("add_provider", { input });
}

export async function updateProvider(
  id: string,
  input: ProviderInput,
): Promise<Provider> {
  return invoke<Provider>("update_provider", { id, input });
}

export async function deleteProvider(id: string): Promise<void> {
  return invoke<void>("delete_provider", { id });
}

// ─── 切换策略 ──────────────────────────────────────────────────────────

export type SwitchStrategy =
  | "priority"
  | "fastest"
  | "cost"
  | "load_balance"
  | "smart";

/** 智能策略 4 个维度的权重；后端要求总和为 100。 */
export interface SmartWeights {
  health: number;
  response_time: number;
  cost: number;
  priority: number;
}

export interface StrategyResponse {
  strategy: SwitchStrategy;
  max_retries: number;
  smart_weights: SmartWeights;
}

export interface StrategyParams {
  max_retries?: number;
  smart_weights?: SmartWeights;
}

export async function getStrategy(): Promise<StrategyResponse> {
  return invoke<StrategyResponse>("get_strategy");
}

export async function setStrategy(strategy: SwitchStrategy): Promise<void> {
  return invoke<void>("set_strategy", { strategy });
}

export async function updateStrategyParams(
  params: StrategyParams,
): Promise<void> {
  return invoke<void>("update_strategy_params", { params });
}

// ─── 健康检查与监控 ────────────────────────────────────────────────────

export type HealthStatus = "healthy" | "degraded" | "down";

export interface HealthSnapshot {
  provider_id: string;
  provider_name: string;
  kind: ProviderInput["kind"];
  status: HealthStatus;
  success_rate: number;
  response_time_us: number | null;
}

export interface HealthSnapshotResponse {
  providers: HealthSnapshot[];
}

/** 读取缓存的健康快照（便宜，秒级查询用）。 */
export async function getHealthSnapshot(): Promise<HealthSnapshotResponse> {
  return invoke<HealthSnapshotResponse>("get_health_snapshot");
}

/** 强制立即对所有 provider 做一次健康探测（用户点"刷新"时调用）。 */
export async function refreshHealthSnapshot(): Promise<HealthSnapshotResponse> {
  return invoke<HealthSnapshotResponse>("refresh_health_snapshot");
}

export interface ProviderStatusChangedEvent {
  provider_id: string;
  provider_name: string;
  old_status: HealthStatus;
  new_status: HealthStatus;
  success_rate: number;
}

/** 镜像 Rust `health::EVENT_PROVIDER_STATUS_CHANGED`。 */
export const EVENT_PROVIDER_STATUS_CHANGED = "provider-status-changed";

/** 后端检测到 healthy↔down 状态切换时推这条事件。 */
export async function onProviderStatusChanged(
  callback: (event: ProviderStatusChangedEvent) => void,
): Promise<UnlistenFn> {
  return listen<ProviderStatusChangedEvent>(
    EVENT_PROVIDER_STATUS_CHANGED,
    (event) => {
      callback(event.payload);
    },
  );
}

// ─── 单个供应商的连接测试 ─────────────────────────────────────────────

export type StreamCheckStatus = "operational" | "degraded" | "failed";

/**
 * 一次连通性 / 流式探针的结构化结果；对齐 cc-switch 的 stream check
 * 语义：除了 latency，还携带 HTTP 状态码、错误分类、实际使用的 model
 * 名等，避免前端只能拿到 number 时各种猜测。
 */
export interface StreamCheckResult {
  status: StreamCheckStatus;
  success: boolean;
  message: string;
  response_time_ms: number | null;
  http_status: number | null;
  model_used: string;
  tested_at: number;
  retry_count: number;
  error_category?: string | null;
}

export async function testProviderConnection(
  id: string,
): Promise<StreamCheckResult> {
  return invoke<StreamCheckResult>("test_provider_connection", { id });
}

// ─── 指标 / 时间序列 / 切换历史 ────────────────────────────────────────

export interface MetricsBucket {
  timestamp: string;
  success_rate: number;
  avg_latency_ms: number;
  p95_latency_ms: number;
  request_count: number;
}

export interface ProviderCostSummary {
  provider_id: string;
  provider_name: string;
  total_tokens: number;
  total_cost: number;
  request_count: number;
}

export interface SwitchEvent {
  id: string;
  timestamp: string;
  from_provider: string;
  to_provider: string;
  strategy: string;
  reason: string;
  details?: string | null;
}

export async function getMetricsTimeseries(
  protocol?: "openai" | "anthropic",
): Promise<MetricsBucket[]> {
  return invoke<MetricsBucket[]>("get_metrics_timeseries", { protocol });
}

export async function getProviderCostSummary(): Promise<ProviderCostSummary[]> {
  return invoke<ProviderCostSummary[]>("get_provider_cost_summary");
}

export async function getSwitchTimeline(): Promise<SwitchEvent[]> {
  return invoke<SwitchEvent[]>("get_switch_timeline");
}

// ─── 桌面通知 ─────────────────────────────────────────────────────────

export async function sendNotification(
  title: string,
  body: string,
): Promise<void> {
  return invoke<void>("send_notification", { title, body });
}

// ─── 配置导入导出 / 模板预设 ──────────────────────────────────────────

export interface ExportProvider {
  name: string;
  kind: ProviderInput["kind"];
  base_url: string;
  priority: number;
  enabled: boolean;
  monthly_quota?: number | null;
  rate_limit_rpm?: number | null;
  cost_per_1k_tokens?: number | null;
}

export interface TemplatePreset {
  id: string;
  name: string;
  description: string;
  providers: ExportProvider[];
}

/**
 * 后端用 `Vec<u8>` 返回二进制 blob，序列化到前端是 `number[]`，这里
 * 包装一层转回 `Uint8Array`，方便直接喂给 `Blob`。
 */
export async function exportConfig(password: string): Promise<Uint8Array> {
  const raw = await invoke<number[]>("export_config_json", { password });
  return new Uint8Array(raw);
}

export async function importConfig(
  data: Uint8Array,
  password: string,
): Promise<void> {
  return invoke<void>("import_config_json", {
    data: Array.from(data),
    password,
  });
}

export async function getTemplatePresets(): Promise<TemplatePreset[]> {
  return invoke<TemplatePreset[]>("get_template_presets");
}

/** 写入剪贴板；Tauri WebView 两端都支持 `navigator.clipboard`。 */
export async function copyToClipboard(text: string): Promise<void> {
  await navigator.clipboard.writeText(text);
}
