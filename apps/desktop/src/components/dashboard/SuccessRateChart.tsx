import {
  CartesianGrid,
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
 * 后端成功率时间序列折线图。后端返回的 `success_rate` 是 0..1，前端
 * 统一展示百分比 —— 转换发生在投影到 chart 数据时，tooltip / Y 轴
 * 都按百分比走。
 */

const REFRESH_INTERVAL_MS = 30_000;

interface ChartPoint {
  time: string;
  fullTime: string;
  successRate: number;
}

export function SuccessRateChart() {
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
        successRate: b.success_rate * 100,
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
      <h4 className="mb-4 text-sm font-medium">
        {t("success_rate_chart_title")}
      </h4>
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
              domain={[0, 100]}
              tickFormatter={(v: number) => `${v}%`}
              tick={{ fontSize: 11 }}
              stroke="hsl(var(--muted-foreground))"
              width={45}
            />
            <Tooltip content={<SuccessTooltip />} />
            <Line
              type="monotone"
              dataKey="successRate"
              stroke="hsl(var(--primary))"
              strokeWidth={2}
              dot={false}
              activeDot={{ r: 4 }}
            />
          </LineChart>
        </ResponsiveContainer>
      )}
    </div>
  );
}

function SuccessTooltip({
  active,
  payload,
}: {
  active?: boolean;
  payload?: { value: number; payload: { fullTime: string } }[];
}) {
  if (!active || !payload?.length) return null;
  const point = payload[0]!;
  return (
    <div className="rounded-lg border border-border bg-card px-3 py-2 text-xs shadow-md">
      <p className="text-muted-foreground">{point.payload.fullTime}</p>
      <p className="mt-1 font-medium">{point.value.toFixed(1)}%</p>
    </div>
  );
}
