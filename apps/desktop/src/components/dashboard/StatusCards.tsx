import { useCallback, useEffect, useState } from "react";
import { Activity, Clock, Server, TrendingUp } from "lucide-react";
import { useTranslation } from "react-i18next";

import { cn } from "@/lib/utils";
import {
  getHealthSnapshot,
  getMetricsTimeseries,
  type HealthSnapshot,
  type MetricsBucket,
} from "@/lib/tauri";

interface CardData {
  currentProvider: string | null;
  todayRequests: number;
  successRate: number | null;
  avgResponseTimeMs: number | null;
}

const REFRESH_INTERVAL = 10_000;

export function StatusCards() {
  const { t } = useTranslation("monitor");
  const [data, setData] = useState<CardData>({
    currentProvider: null,
    todayRequests: 0,
    successRate: null,
    avgResponseTimeMs: null,
  });
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback(async () => {
    try {
      const [healthRes, metrics] = await Promise.all([
        getHealthSnapshot(),
        getMetricsTimeseries(),
      ]);

      const providers: HealthSnapshot[] = healthRes.providers;

      const activeProvider =
        providers.find((p) => p.status === "healthy") ?? providers[0] ?? null;

      const totalRequests = metrics.reduce(
        (sum: number, b: MetricsBucket) => sum + b.request_count,
        0,
      );

      let overallSuccessRate: number | null = null;
      if (metrics.length > 0) {
        const totalSuccess = metrics.reduce(
          (sum: number, b: MetricsBucket) =>
            sum + b.success_rate * b.request_count,
          0,
        );
        overallSuccessRate =
          totalRequests > 0 ? totalSuccess / totalRequests : null;
      }

      let avgLatency: number | null = null;
      if (metrics.length > 0) {
        const totalLatency = metrics.reduce(
          (sum: number, b: MetricsBucket) =>
            sum + b.avg_latency_ms * b.request_count,
          0,
        );
        avgLatency = totalRequests > 0 ? totalLatency / totalRequests : null;
      }

      setData({
        currentProvider: activeProvider?.provider_name ?? null,
        todayRequests: totalRequests,
        successRate: overallSuccessRate,
        avgResponseTimeMs: avgLatency,
      });
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

  return (
    <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
      <StatCard
        icon={<Server className="size-4" />}
        label={t("current_provider")}
        value={loading ? "--" : (data.currentProvider ?? t("none"))}
        loading={loading}
      />
      <StatCard
        icon={<Activity className="size-4" />}
        label={t("today_requests")}
        value={loading ? "--" : String(data.todayRequests)}
        loading={loading}
      />
      <StatCard
        icon={<TrendingUp className="size-4" />}
        label={t("success_rate")}
        value={
          loading
            ? "--"
            : data.successRate != null
              ? `${(data.successRate * 100).toFixed(1)}%`
              : "--"
        }
        loading={loading}
        highlight={
          data.successRate != null && data.successRate < 0.95
            ? "warning"
            : undefined
        }
      />
      <StatCard
        icon={<Clock className="size-4" />}
        label={t("avg_response_time")}
        value={
          loading
            ? "--"
            : data.avgResponseTimeMs != null
              ? `${Math.round(data.avgResponseTimeMs)}ms`
              : "--"
        }
        loading={loading}
      />
    </div>
  );
}

interface StatCardProps {
  icon: React.ReactNode;
  label: string;
  value: string;
  loading: boolean;
  highlight?: "warning" | "error";
}

function StatCard({ icon, label, value, loading, highlight }: StatCardProps) {
  return (
    <div className="rounded-xl border border-border bg-card p-4 shadow-sm">
      <div className="flex items-center gap-2 text-muted-foreground">
        {icon}
        <span className="text-xs uppercase tracking-wide">{label}</span>
      </div>
      <p
        className={cn(
          "mt-2 text-xl font-semibold tabular-nums tracking-tight",
          loading && "animate-pulse text-muted-foreground",
          highlight === "warning" && "text-yellow-600",
          highlight === "error" && "text-destructive",
        )}
      >
        {value}
      </p>
    </div>
  );
}
