import { LocalApiCard } from "@/components/local-api/LocalApiCard";
import { StatusCards } from "@/components/dashboard/StatusCards";
import { SuccessRateChart } from "@/components/dashboard/SuccessRateChart";
import { LatencyChart } from "@/components/dashboard/LatencyChart";
import { CostPieChart } from "@/components/dashboard/CostPieChart";
import { SwitchTimeline } from "@/components/dashboard/SwitchTimeline";

export function DashboardPage() {
  return (
    <section className="space-y-6">
      <h2 className="text-2xl font-semibold leading-apple-headline tracking-apple-tight">
        Dashboard
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
