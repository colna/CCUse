import { useCallback, useEffect, useState } from "react";
import {
  ApartmentOutlined,
  ClockCircleOutlined,
  ReloadOutlined,
  CloudServerOutlined,
  RiseOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import {
  getHealthSnapshot,
  getMetricsTimeseries,
  getStrategy,
  onProviderStatusChanged,
  refreshHealthSnapshot,
  type HealthSnapshot,
  type MetricsBucket,
  type SwitchStrategy,
} from "@/lib/tauri";

interface CardData {
  currentProvider: string | null;
  todayRequests: number;
  successRate: number | null;
  avgResponseTimeMs: number | null;
}

const REFRESH_INTERVAL = 10_000;

async function loadHealthSnapshot(forceRefresh: boolean) {
  return forceRefresh ? refreshHealthSnapshot() : getHealthSnapshot();
}

function selectCurrentProvider(
  providers: HealthSnapshot[],
  strategy: SwitchStrategy,
): HealthSnapshot | null {
  const aliveProviders = providers.filter((p) => p.status !== "down");
  if (aliveProviders.length === 0) {
    return null;
  }

  if (strategy === "fastest") {
    const providersWithLatency = aliveProviders.filter(
      (p): p is HealthSnapshot & { response_time_us: number } =>
        p.response_time_us != null,
    );

    if (providersWithLatency.length > 0) {
      return providersWithLatency.reduce((best, provider) =>
        provider.response_time_us < best.response_time_us ? provider : best,
      );
    }
  }

  return (
    aliveProviders.find((p) => p.status === "healthy") ?? aliveProviders[0]
  );
}

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
  const [refreshing, setRefreshing] = useState(false);

  const fetchData = useCallback(async (forceRefresh = false) => {
    try {
      const [healthRes, metrics, strategy] = await Promise.all([
        loadHealthSnapshot(forceRefresh),
        getMetricsTimeseries(),
        getStrategy(),
      ]);

      const providers: HealthSnapshot[] = healthRes.providers;
      const activeProvider = selectCurrentProvider(
        providers,
        strategy.strategy,
      );

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
    const id = setInterval(fetchData, REFRESH_INTERVAL);
    return () => clearInterval(id);
  }, [fetchData]);

  useEffect(() => {
    const unlistenPromise = onProviderStatusChanged(() => {
      void fetchData();
    }).catch(() => null);

    return () => {
      void unlistenPromise.then((unlisten) => unlisten?.());
    };
  }, [fetchData]);

  if (error) {
    return (
      <div className="rounded-xl border border-destructive/30 bg-card p-4 text-sm text-destructive">
        {error}
      </div>
    );
  }

  return (
    <section className="space-y-4">
      <div className="flex items-center justify-between gap-3">
        <h3 className="text-sm font-semibold text-foreground">
          {t("status_overview")}
        </h3>
        <Button
          htmlType="button"
          size="small"
          type="default"
          onClick={handleRefresh}
          disabled={refreshing}
          icon={
            <ReloadOutlined
              spin={refreshing}
              aria-label=""
              role="presentation"
            />
          }
        >
          {t("refresh")}
        </Button>
      </div>
      <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
        <StatCard
          testId="current-provider-card"
          icon={<CloudServerOutlined aria-label="" role="presentation" />}
          label={t("current_provider")}
          value={loading ? "--" : (data.currentProvider ?? t("none"))}
          loading={loading}
        />
        <StatCard
          testId="today-requests-card"
          icon={<ApartmentOutlined aria-label="" role="presentation" />}
          label={t("today_requests")}
          value={loading ? "--" : String(data.todayRequests)}
          loading={loading}
        />
        <StatCard
          testId="success-rate-card"
          icon={<RiseOutlined aria-label="" role="presentation" />}
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
          icon={<ClockCircleOutlined aria-label="" role="presentation" />}
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
    </section>
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
      className="rounded-2xl border border-[var(--ant-color-border-secondary,rgba(0,0,0,0.06))] bg-[var(--ant-color-bg-container,#fff)] p-5"
    >
      <div className="flex items-center gap-2 text-muted-foreground">
        {icon}
        <span className="text-xs uppercase tracking-wide">{label}</span>
      </div>
      <p
        data-testid={`${testId}-value`}
        className={cn(
          "mt-2 text-[26px] font-semibold tabular-nums leading-tight tracking-tight",
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
