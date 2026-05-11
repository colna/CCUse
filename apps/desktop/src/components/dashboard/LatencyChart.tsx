import {
  CartesianGrid,
  Legend,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { useMemo } from "react";
import { useTranslation } from "react-i18next";

import { formatFullTime, formatShortTime } from "@/lib/timeFormat";
import { getMetricsTimeseries } from "@/lib/tauri";
import { useTimeseriesPoll } from "@/lib/useTimeseriesPoll";

/**
 * 后端延迟时间序列折线图。同时画"均值"和"p95"两条线，p95 用 dashed
 * 区分；颜色固定使用 antd primary + 视觉互补的橙色，避免依赖外部
 * 主题里没有的"warning"色。
 */

const REFRESH_INTERVAL_MS = 30_000;

interface ChartPoint {
  time: string;
  fullTime: string;
  avgLatency: number;
  p95Latency: number;
}

export function LatencyChart() {
  const { t } = useTranslation("monitor");
  const { t: tc } = useTranslation("common");
  const { data, loading, error } = useTimeseriesPoll(
    getMetricsTimeseries,
    REFRESH_INTERVAL_MS,
  );

  const chartData = useMemo<ChartPoint[]>(
    () =>
      (data ?? []).map((b) => ({
        time: formatShortTime(b.timestamp),
        fullTime: formatFullTime(b.timestamp),
        avgLatency: b.avg_latency_ms,
        p95Latency: b.p95_latency_ms,
      })),
    [data],
  );

  if (error) {
    return (
      <div className="border-destructive/30 rounded-xl border bg-card p-4 text-sm text-destructive">
        {error}
      </div>
    );
  }

  if (!loading && chartData.length === 0) {
    return (
      <div className="bg-card/50 rounded-xl border border-dashed border-border px-6 py-8 text-center text-sm text-muted-foreground">
        {tc("no_data_yet")}
      </div>
    );
  }

  return (
    <div className="rounded-xl border border-border bg-card p-4 shadow-sm">
      <h4 className="mb-4 text-sm font-medium">{t("latency_chart_title")}</h4>
      {loading ? (
        <div className="flex h-48 items-center justify-center text-sm text-muted-foreground">
          {tc("loading")}
        </div>
      ) : (
        <ResponsiveContainer width="100%" height={200}>
          <LineChart data={chartData}>
            <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
            <XAxis
              dataKey="time"
              tick={{ fontSize: 11 }}
              stroke="hsl(var(--muted-foreground))"
            />
            <YAxis
              tickFormatter={(v: number) => `${v}ms`}
              tick={{ fontSize: 11 }}
              stroke="hsl(var(--muted-foreground))"
              width={55}
            />
            <Tooltip content={<LatencyTooltip t={t} />} />
            <Legend wrapperStyle={{ fontSize: 12 }} />
            <Line
              type="monotone"
              dataKey="avgLatency"
              name={t("latency_avg")}
              stroke="hsl(var(--primary))"
              strokeWidth={2}
              dot={false}
              activeDot={{ r: 4 }}
            />
            <Line
              type="monotone"
              dataKey="p95Latency"
              name={t("latency_p95")}
              stroke="hsl(24 100% 50%)"
              strokeWidth={2}
              strokeDasharray="4 2"
              dot={false}
              activeDot={{ r: 4 }}
            />
          </LineChart>
        </ResponsiveContainer>
      )}
    </div>
  );
}

function LatencyTooltip({
  active,
  payload,
  t,
}: {
  active?: boolean;
  payload?: {
    payload: { fullTime: string; avgLatency: number; p95Latency: number };
  }[];
  t: (key: string, opts?: Record<string, string | number>) => string;
}) {
  if (!active || !payload?.length) return null;
  const data = payload[0]!.payload;
  return (
    <div className="rounded-lg border border-border bg-card px-3 py-2 text-xs shadow-md">
      <p className="text-muted-foreground">{data.fullTime}</p>
      <p className="mt-1">
        <span className="mr-1.5 inline-block h-0.5 w-3 bg-primary align-middle" />
        {t("latency_avg_value", { value: Math.round(data.avgLatency) })}
      </p>
      <p>
        <span className="mr-1.5 inline-block h-0.5 w-3 bg-orange-500 align-middle" />
        {t("latency_p95_value", { value: Math.round(data.p95Latency) })}
      </p>
    </div>
  );
}
