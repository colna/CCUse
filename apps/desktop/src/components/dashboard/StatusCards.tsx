import { useCallback, useEffect, useState } from "react";
import { Activity, Clock, RefreshCw, Server, TrendingUp } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import {
  getCurrentProvider,
  getHealthSnapshot,
  getMetricsTimeseries,
  refreshHealthSnapshot,
  type CurrentProviderSnapshot,
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
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback(async (forceProbe = false) => {
    try {
      const [healthRes, metrics, current] = await Promise.all([
        forceProbe ? refreshHealthSnapshot() : getHealthSnapshot(),
        getMetricsTimeseries(),
        getCurrentProvider(),
      ]);

      const providers: HealthSnapshot[] = healthRes.providers;
      const currentProvider: CurrentProviderSnapshot = current;

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
        currentProvider:
          currentProvider.provider_name ??
          activeProvider?.provider_name ??
          null,
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

  const handleRefresh = useCallback(async () => {
    setRefreshing(true);
    try {
      await fetchData(true);
    } finally {
      setRefreshing(false);
    }
  }, [fetchData]);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  useEffect(() => {
    const id = setInterval(() => {
      void fetchData();
    }, REFRESH_INTERVAL);
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
    <div className="space-y-3">
      <div className="flex justify-end">
        <Button
          variant="outline"
          size="sm"
          onClick={handleRefresh}
          disabled={refreshing}
          aria-label={t("refresh_current_provider_aria")}
          title={t("refresh_current_provider_aria")}
        >
          <RefreshCw
            className={cn("mr-1.5 size-3.5", refreshing && "animate-spin")}
          />
          {t("refresh_current_provider")}
        </Button>
      </div>

      <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
        <StatCard
          testId="current-provider-card"
          icon={<Server className="size-4" />}
          label={t("current_provider")}
          value={loading ? "--" : (data.currentProvider ?? t("none"))}
          loading={loading}
        />
        <StatCard
          testId="today-requests-card"
          icon={<Activity className="size-4" />}
          label={t("today_requests")}
          value={loading ? "--" : String(data.todayRequests)}
          loading={loading}
        />
        <StatCard
          testId="success-rate-card"
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
          testId="avg-response-time-card"
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
    </div>
  );
}

interface StatCardProps {
  testId: string;
  icon: React.ReactNode;
  label: string;
  value: string;
  loading: boolean;
  highlight?: "warning" | "error";
}

function StatCard({
  testId,
  icon,
  label,
  value,
  loading,
  highlight,
}: StatCardProps) {
  return (
    <div
      data-testid={testId}
      className="rounded-xl border border-border bg-card p-4 shadow-sm"
    >
      <div className="flex items-center gap-2 text-muted-foreground">
        {icon}
        <span className="text-xs uppercase tracking-wide">{label}</span>
      </div>
      <p
        data-testid={`${testId}-value`}
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
