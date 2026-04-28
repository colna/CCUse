import { useCallback, useEffect, useState } from "react";
import {
  PieChart,
  Pie,
  Cell,
  Tooltip,
  Legend,
  ResponsiveContainer,
} from "recharts";
import { useTranslation } from "react-i18next";

import { getProviderCostSummary, type ProviderCostSummary } from "@/lib/tauri";

interface ChartSlice {
  name: string;
  value: number;
  requests: number;
}

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

function CustomTooltip({
  active,
  payload,
  t,
}: {
  active?: boolean;
  payload?: { name: string; value: number; payload: { requests: number } }[];
  t: (key: string, opts?: Record<string, string | number>) => string;
}) {
  if (!active || !payload?.length) return null;
  const entry = payload[0];
  return (
    <div className="rounded-lg border border-border bg-card px-3 py-2 text-xs shadow-md">
      <p className="font-medium">{entry.name}</p>
      <p className="mt-1 text-muted-foreground">
        {t("cost_label", { value: (entry.value as number).toFixed(4) })}
      </p>
      <p className="text-muted-foreground">
        {t("requests_label", { value: entry.payload.requests })}
      </p>
    </div>
  );
}

const REFRESH_INTERVAL = 30_000;

export function CostPieChart() {
  const { t } = useTranslation("monitor");
  const { t: tc } = useTranslation("common");
  const [chartData, setChartData] = useState<ChartSlice[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback(async () => {
    try {
      const summaries: ProviderCostSummary[] = await getProviderCostSummary();
      const slices: ChartSlice[] = summaries.map((s) => ({
        name: s.provider_name,
        value: s.total_cost,
        requests: s.request_count,
      }));
      setChartData(slices);
      setError(null);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  useEffect(() => {
    const id = setInterval(fetchData, REFRESH_INTERVAL);
    return () => clearInterval(id);
  }, [fetchData]);

  if (error) {
    return (
      <div className="rounded-xl border border-destructive/30 bg-card p-4 text-sm text-destructive">
        {error}
      </div>
    );
  }

  if (!loading && chartData.length === 0) {
    return (
      <div className="rounded-xl border border-dashed border-border bg-card/50 px-6 py-8 text-center text-sm text-muted-foreground">
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
            <Tooltip content={<CustomTooltip t={t} />} />
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
