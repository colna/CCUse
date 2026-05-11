import { useTranslation } from "react-i18next";

import { CostPieChart } from "@/components/dashboard/CostPieChart";
import { LatencyChart } from "@/components/dashboard/LatencyChart";
import { StatusCards } from "@/components/dashboard/StatusCards";
import { SuccessRateChart } from "@/components/dashboard/SuccessRateChart";
import { SwitchTimeline } from "@/components/dashboard/SwitchTimeline";
import { LocalApiCard } from "@/components/local-api/LocalApiCard";

/**
 * 仪表盘页面：
 *   StatusCards         — 顶部 4 张关键指标卡
 *   LocalApiCard        — 本地代理 Base URL/Key
 *   2x2 折线 + 甜甜圈 + 切换历史
 *
 * 每个块都自己拉数据 + 自己刷新；这样某个 API 挂掉时只影响那一块的
 * 错误态，不会把整个页面打挂。
 */
export function DashboardPage() {
  const { t } = useTranslation("common");

  return (
    <section className="space-y-6">
      <h2 className="text-2xl font-semibold leading-apple-headline tracking-apple-tight">
        {t("page_dashboard_title")}
      </h2>

      <StatusCards />
      <LocalApiCard />

      <div className="grid gap-4 lg:grid-cols-2">
        <SuccessRateChart />
        <LatencyChart />
      </div>

      <div className="grid gap-4 lg:grid-cols-2">
        <CostPieChart />
        <SwitchTimeline />
      </div>
    </section>
  );
}
