import {
  Cell,
  Legend,
  Pie,
  PieChart,
  ResponsiveContainer,
  Tooltip,
} from "recharts";
import { useMemo } from "react";
import { useTranslation } from "react-i18next";

import { getProviderCostSummary } from "@/lib/tauri";
import { useTimeseriesPoll } from "@/lib/useTimeseriesPoll";

/**
 * 按供应商展示成本分布的甜甜圈图。颜色环来自常见品牌色调，避免靠
 * 主题变量决定（pie 图需要稳定可识别的多色）。
 */

const REFRESH_INTERVAL_MS = 30_000;

const COLORS = [
  "hsl(var(--primary))",
  "hsl(24 100% 50%)",
  "hsl(142 70% 45%)",
  "hsl(280 65% 55%)",
  "hsl(200 70% 50%)",
  "hsl(340 75% 55%)",
  "hsl(55 80% 50%)",
  "hsl(170 60% 45%)",
];

interface ChartSlice {
  name: string;
  value: number;
  requests: number;
}

export function CostPieChart() {
  const { t } = useTranslation("monitor");
  const { t: tc } = useTranslation("common");
  const { data, loading, error } = useTimeseriesPoll(
    getProviderCostSummary,
    REFRESH_INTERVAL_MS,
  );

  const chartData = useMemo<ChartSlice[]>(
    () =>
      (data ?? []).map((s) => ({
        name: s.provider_name,
        value: s.total_cost,
        requests: s.request_count,
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
      <h4 className="mb-4 text-sm font-medium">{t("cost_chart_title")}</h4>
      {loading ? (
        <div className="flex h-48 items-center justify-center text-sm text-muted-foreground">
          {tc("loading")}
        </div>
      ) : (
        <ResponsiveContainer width="100%" height={240}>
          <PieChart>
            <Pie
              data={chartData}
              cx="50%"
              cy="50%"
              innerRadius={50}
              outerRadius={80}
              paddingAngle={2}
              dataKey="value"
              nameKey="name"
            >
              {chartData.map((_entry, index) => (
                <Cell
                  key={`cell-${index}`}
                  fill={COLORS[index % COLORS.length]}
                />
              ))}
            </Pie>
            <Tooltip content={<CostTooltip t={t} />} />
            <Legend
              wrapperStyle={{ fontSize: 12 }}
              formatter={(value: string) => (
                <span className="text-xs text-foreground">{value}</span>
              )}
            />
          </PieChart>
        </ResponsiveContainer>
      )}
    </div>
  );
}

function CostTooltip({
  active,
  payload,
  t,
}: {
  active?: boolean;
  payload?: { name: string; value: number; payload: { requests: number } }[];
  t: (key: string, opts?: Record<string, string | number>) => string;
}) {
  if (!active || !payload?.length) return null;
  const entry = payload[0]!;
  return (
    <div className="rounded-lg border border-border bg-card px-3 py-2 text-xs shadow-md">
      <p className="font-medium">{entry.name}</p>
      <p className="mt-1 text-muted-foreground">
        {t("cost_label", { value: entry.value.toFixed(4) })}
      </p>
      <p className="text-muted-foreground">
        {t("requests_label", { value: entry.payload.requests })}
      </p>
    </div>
  );
}
