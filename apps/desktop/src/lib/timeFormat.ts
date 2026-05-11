/**
 * 仪表盘里通用的图表/时间序列时间戳格式化。
 *
 * 抽出来是因为 SuccessRate/Latency 两张折线图与未来可能加入的图表
 * 都用同样的格式（x 轴只看小时:分钟，tooltip 显示月日时分），与其每个
 * 组件各写一份不如固化在一处。
 */

const SHORT_TIME: Intl.DateTimeFormatOptions = {
  hour: "2-digit",
  minute: "2-digit",
};

const FULL_TIME: Intl.DateTimeFormatOptions = {
  month: "short",
  day: "numeric",
  hour: "2-digit",
  minute: "2-digit",
};

export function formatShortTime(timestamp: string): string {
  return new Date(timestamp).toLocaleTimeString([], SHORT_TIME);
}

export function formatFullTime(timestamp: string): string {
  return new Date(timestamp).toLocaleString([], FULL_TIME);
}
