import type { HealthStatus, StreamCheckResult } from "@/lib/tauri";

/**
 * `ProviderList` 行内健康徽章/标签的展示助手。
 *
 * 后端通过两个语义不同但视觉上重叠的概念描述供应商状态：
 * - `HealthStatus`：sliding-window 上聚合出的近期健康度（healthy /
 *   degraded / down）；
 * - `StreamCheckResult["status"]`：单次连通性探针的结果
 *   （operational / degraded / failed）。
 *
 * 这里把这两套词汇都映射到同一份"运行中 / 降级 / 失败"中文文案，避免
 * 在 UI 上同时出现两种描述。
 */

/** 健康状态点的颜色 class —— 用 Tailwind 的预设色块，跟随主题。 */
export function statusDotColor(status?: HealthStatus): string {
  switch (status) {
    case "healthy":
      return "bg-green-500";
    case "degraded":
      return "bg-yellow-500";
    case "down":
      return "bg-red-500";
    default:
      return "bg-muted-foreground/40";
  }
}

/** `0.987 → "98.7%"`；null/undefined 时回退占位符。 */
export function formatSuccessRate(rate?: number): string {
  if (rate == null) return "--";
  return `${(rate * 100).toFixed(1)}%`;
}

/** 单次 stream check 状态 → 中文短标签。 */
export function streamStatusLabel(
  status?: StreamCheckResult["status"],
): string {
  switch (status) {
    case "operational":
      return "正常";
    case "degraded":
      return "降级";
    case "failed":
      return "失败";
    default:
      return "--";
  }
}

/** 把 `HealthStatus` 收敛成 stream check 那一套词汇后再走 `streamStatusLabel`。 */
export function healthStatusLabel(status?: HealthStatus): string {
  if (!status) return "--";
  return streamStatusLabel(
    status === "healthy"
      ? "operational"
      : status === "degraded"
        ? "degraded"
        : "failed",
  );
}
