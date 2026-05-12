import { useCallback, useEffect, useState } from "react";
import {
  ApartmentOutlined,
  ClockCircleOutlined,
  ReloadOutlined,
  CloudServerOutlined,
  RiseOutlined,
} from "@ant-design/icons";
import { Segmented } from "antd";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { protocolForKind, type ProtocolGroup } from "@/lib/providerKinds";
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

/**
 * 仪表盘顶部的 4 张关键指标卡：当前供应商 / 今日请求 / 成功率 / 平均
 * 响应。Segmented 控件按 OpenAI / Anthropic 协议切换 — 数据源、
 * provider 过滤、当前 provider 选择全部按所选协议重算。
 *
 * "当前供应商"的判定与后端 SwitchEngine 的策略保持视觉一致：fastest
 * 时取响应最快的健康节点，否则取第一个 healthy 节点。它只用于显示，
 * 真正的路由由后端决定。
 */

const REFRESH_INTERVAL_MS = 10_000;

interface CardData {
  currentProvider: string | null;
  todayRequests: number;
  successRate: number | null;
  avgResponseTimeMs: number | null;
}

const EMPTY_CARD: CardData = {
  currentProvider: null,
  todayRequests: 0,
  successRate: null,
  avgResponseTimeMs: null,
};

export function StatusCards() {
  const { t } = useTranslation("monitor");
  const [data, setData] = useState<CardData>(EMPTY_CARD);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [activeProtocol, setActiveProtocol] = useState<ProtocolGroup>("openai");

  const fetchData = useCallback(
    async (forceRefresh = false) => {
      try {
        const [healthRes, metrics, strategy] = await Promise.all([
          forceRefresh ? refreshHealthSnapshot() : getHealthSnapshot(),
          getMetricsTimeseries(activeProtocol),
          getStrategy(),
        ]);
        const scoped = healthRes.providers.filter(
          (p) => protocolForKind(p.kind) === activeProtocol,
        );
        setData(deriveCardData(scoped, metrics, strategy.strategy));
        setError(null);
      } catch (err: unknown) {
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        setLoading(false);
      }
    },
    [activeProtocol],
  );

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
    const id = setInterval(fetchData, REFRESH_INTERVAL_MS);
    return () => clearInterval(id);
  }, [fetchData]);

  useEffect(() => {
    // `.catch(() => undefined)` 消化非 Tauri 环境下 `listen()` 抛错；
    // 见 `ProviderList`。
    const unlistenPromise = onProviderStatusChanged(() => {
      void fetchData();
    }).catch(() => undefined);
    return () => {
      void unlistenPromise.then((unlisten) => unlisten?.());
    };
  }, [fetchData]);

  if (error) {
    return (
      <div className="border-destructive/30 rounded-xl border bg-card p-4 text-sm text-destructive">
        {error}
      </div>
    );
  }

  return (
    <section className="space-y-4">
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-3">
          <h3 className="text-sm font-semibold text-foreground">
            {t("status_overview")}
          </h3>
          <Segmented
            size="small"
            value={activeProtocol}
            onChange={(value) => setActiveProtocol(value as ProtocolGroup)}
            options={[
              {
                label: t("status_overview_protocol_openai"),
                value: "openai",
              },
              {
                label: t("status_overview_protocol_anthropic"),
                value: "anthropic",
              },
            ]}
            aria-label={t("status_overview_protocol_aria")}
          />
        </div>
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
      className="rounded-2xl border border-[var(--app-border-secondary)] bg-[var(--app-bg-container)] p-5"
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

/** 把 metrics 时间序列折算成"总请求加权平均"，与下面图表口径一致。 */
function deriveCardData(
  providers: HealthSnapshot[],
  metrics: MetricsBucket[],
  strategy: SwitchStrategy,
): CardData {
  const totalRequests = metrics.reduce((sum, b) => sum + b.request_count, 0);
  const weightedAvg = (pick: (b: MetricsBucket) => number): number | null => {
    if (metrics.length === 0 || totalRequests === 0) return null;
    return (
      metrics.reduce((sum, b) => sum + pick(b) * b.request_count, 0) /
      totalRequests
    );
  };

  const activeProvider = selectCurrentProvider(providers, strategy);
  return {
    currentProvider: activeProvider?.provider_name ?? null,
    todayRequests: totalRequests,
    successRate: weightedAvg((b) => b.success_rate),
    avgResponseTimeMs: weightedAvg((b) => b.avg_latency_ms),
  };
}

function selectCurrentProvider(
  providers: HealthSnapshot[],
  strategy: SwitchStrategy,
): HealthSnapshot | null {
  const alive = providers.filter((p) => p.status !== "down");
  if (alive.length === 0) return null;

  if (strategy === "fastest") {
    const withLatency = alive.filter(
      (p): p is HealthSnapshot & { response_time_us: number } =>
        p.response_time_us != null,
    );
    if (withLatency.length > 0) {
      return withLatency.reduce((best, p) =>
        p.response_time_us < best.response_time_us ? p : best,
      );
    }
  }

  return alive.find((p) => p.status === "healthy") ?? alive[0]!;
}
