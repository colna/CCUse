import { useCallback, useEffect, useState } from "react";
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from "recharts";

import { getMetricsTimeseries, type MetricsBucket } from "@/lib/tauri";

interface ChartPoint {
  time: string;
  fullTime: string;
  successRate: number;
}

function formatTime(timestamp: string): string {
  const d = new Date(timestamp);
  return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

function formatFullTime(timestamp: string): string {
  const d = new Date(timestamp);
  return d.toLocaleString([], {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function CustomTooltip({
  active,
  payload,
}: {
  active?: boolean;
  payload?: any[];
}) {
  if (!active || !payload?.length) return null;
  const point = payload[0];
  return (
    <div className="rounded-lg border border-border bg-card px-3 py-2 text-xs shadow-md">
      <p className="text-muted-foreground">{point.payload.fullTime}</p>
      <p className="mt-1 font-medium">{(point.value as number).toFixed(1)}%</p>
    </div>
  );
}

const REFRESH_INTERVAL = 30_000;

export function SuccessRateChart() {
  const [chartData, setChartData] = useState<ChartPoint[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback(async () => {
    try {
      const buckets: MetricsBucket[] = await getMetricsTimeseries();
      const points: ChartPoint[] = buckets.map((b) => ({
        time: formatTime(b.timestamp),
        fullTime: formatFullTime(b.timestamp),
        successRate: b.success_rate * 100,
      }));
      setChartData(points);
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
        No data yet
      </div>
    );
  }

  return (
    <div className="rounded-xl border border-border bg-card p-4 shadow-sm">
      <h4 className="mb-4 text-sm font-medium">Success Rate (24h)</h4>
      {loading ? (
        <div className="flex h-48 items-center justify-center text-sm text-muted-foreground">
          Loading...
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
            <Tooltip content={<CustomTooltip />} />
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
