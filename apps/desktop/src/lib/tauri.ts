import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Wire shape returned by the `get_local_api_config` Tauri command.
 * Mirrors `proxy::runtime::LocalApiConfig` in the Rust backend. */
export interface LocalApiConfig {
  base_url: string;
  api_key: string;
}

/** Caller-supplied provider input. Mirrors
 * `providers::model::ProviderInput`; backend persistence lands in
 * T1.0.2.19's `add_provider` Tauri command. */
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

/** Persisted provider returned by `list_providers` / `add_provider`. */
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

// ─── Provider CRUD (T1.0.2.19) ───────────────────────────────

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

// ─── Strategy (T1.0.2.20) ────────────────────────────────────

export type SwitchStrategy =
  | "priority"
  | "fastest"
  | "cost"
  | "load_balance"
  | "smart";

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

// ─── Health (T1.0.2.21) ──────────────────────────────────────

export type HealthStatus = "healthy" | "degraded" | "down";

export interface HealthSnapshot {
  provider_id: string;
  provider_name: string;
  status: HealthStatus;
  success_rate: number;
  response_time_us: number | null;
}

export interface HealthSnapshotResponse {
  providers: HealthSnapshot[];
}

export async function getHealthSnapshot(): Promise<HealthSnapshotResponse> {
  return invoke<HealthSnapshotResponse>("get_health_snapshot");
}

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

/** Stable event name; mirrors `health::EVENT_PROVIDER_STATUS_CHANGED`
 * on the Rust side. */
export const EVENT_PROVIDER_STATUS_CHANGED = "provider-status-changed";

/** Subscribe to health status changes emitted by the backend checker. */
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

// ─── Model Mapping (T1.0.3.12) ──────────────────────────────

export interface MappingEntry {
  client_model: string;
  openai: string | null;
  anthropic: string | null;
  gemini: string | null;
}

export async function getModelMappings(): Promise<MappingEntry[]> {
  return invoke<MappingEntry[]>("get_model_mappings");
}

export async function setModelMapping(
  clientModel: string,
  vendor: string,
  vendorModel: string,
): Promise<void> {
  return invoke<void>("set_model_mapping", {
    clientModel,
    vendor,
    vendorModel,
  });
}

export async function removeModelMapping(
  clientModel: string,
  vendor: string,
): Promise<void> {
  return invoke<void>("remove_model_mapping", { clientModel, vendor });
}

/** Stable event name; mirrors `commands::EVENT_LOCAL_API_CONFIG_CHANGED`
 * on the Rust side. Pinned in tests on both sides. */
export const EVENT_LOCAL_API_CONFIG_CHANGED = "local_api_config_changed";

/** Subscribe to proxy config changes (regenerate / restart).
 * Returns the unlisten function — wire it into a useEffect cleanup. */
export async function onLocalApiConfigChanged(
  callback: (config: LocalApiConfig) => void,
): Promise<UnlistenFn> {
  return listen<LocalApiConfig>(EVENT_LOCAL_API_CONFIG_CHANGED, (event) => {
    callback(event.payload);
  });
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

// ─── Connection Test (T1.0.4.05) ─────────────────────────────

export async function testProviderConnection(id: string): Promise<number> {
  return invoke<number>("test_provider_connection", { id });
}

// ─── Metrics & Timeline (T1.0.4.10-13) ──────────────────────

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

export async function getMetricsTimeseries(): Promise<MetricsBucket[]> {
  return invoke<MetricsBucket[]>("get_metrics_timeseries");
}

export async function getProviderCostSummary(): Promise<ProviderCostSummary[]> {
  return invoke<ProviderCostSummary[]>("get_provider_cost_summary");
}

export async function getSwitchTimeline(): Promise<SwitchEvent[]> {
  return invoke<SwitchEvent[]>("get_switch_timeline");
}

// ─── Notification (T1.0.4.17) ────────────────────────────────

export async function sendNotification(
  title: string,
  body: string,
): Promise<void> {
  return invoke<void>("send_notification", { title, body });
}

// ─── Config Export / Import (T1.0.4.18-20) ───────────────────

export interface ExportProvider {
  name: string;
  kind: "openai" | "anthropic" | "gemini" | "relay" | "custom";
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

/** Copy text to the system clipboard.
 *
 * Browser Clipboard API is the happy path; we keep the
 * `document.execCommand` fallback because Tauri's webview on
 * older WebKit / WebView2 builds occasionally exposes neither. */
export async function copyToClipboard(text: string): Promise<void> {
  if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    return;
  }
  if (typeof document === "undefined") return;
  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.setAttribute("readonly", "");
  textarea.style.position = "fixed";
  document.body.appendChild(textarea);
  textarea.select();
  document.execCommand("copy");
  document.body.removeChild(textarea);
}
