import { useCallback, useEffect, useState } from "react";
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  Legend,
  ResponsiveContainer,
} from "recharts";

import { getMetricsTimeseries, type MetricsBucket } from "@/lib/tauri";

interface ChartPoint {
  time: string;
  fullTime: string;
  avgLatency: number;
  p95Latency: number;
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

function CustomTooltip({
  active,
  payload,
}: {
  active?: boolean;
  payload?: {
    payload: { fullTime: string; avgLatency: number; p95Latency: number };
  }[];
}) {
  if (!active || !payload?.length) return null;
  const data = payload[0]?.payload;
  return (
    <div className="rounded-lg border border-border bg-card px-3 py-2 text-xs shadow-md">
      <p className="text-muted-foreground">{data.fullTime}</p>
      <p className="mt-1">
        <span className="mr-1.5 inline-block h-0.5 w-3 bg-primary align-middle" />
        Avg: {Math.round(data.avgLatency)}ms
      </p>
      <p>
        <span className="mr-1.5 inline-block h-0.5 w-3 bg-orange-500 align-middle" />
        P95: {Math.round(data.p95Latency)}ms
      </p>
    </div>
  );
}

const REFRESH_INTERVAL = 30_000;

export function LatencyChart() {
  const [chartData, setChartData] = useState<ChartPoint[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback(async () => {
    try {
      const buckets: MetricsBucket[] = await getMetricsTimeseries();
      const points: ChartPoint[] = buckets.map((b) => ({
        time: formatTime(b.timestamp),
        fullTime: formatFullTime(b.timestamp),
        avgLatency: b.avg_latency_ms,
        p95Latency: b.p95_latency_ms,
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
      <h4 className="mb-4 text-sm font-medium">Response Time (24h)</h4>
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
              tickFormatter={(v: number) => `${v}ms`}
              tick={{ fontSize: 11 }}
              stroke="hsl(var(--muted-foreground))"
              width={55}
            />
            <Tooltip content={<CustomTooltip />} />
            <Legend wrapperStyle={{ fontSize: 12 }} />
            <Line
              type="monotone"
              dataKey="avgLatency"
              name="Avg"
              stroke="hsl(var(--primary))"
              strokeWidth={2}
              dot={false}
              activeDot={{ r: 4 }}
            />
            <Line
              type="monotone"
              dataKey="p95Latency"
              name="P95"
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
