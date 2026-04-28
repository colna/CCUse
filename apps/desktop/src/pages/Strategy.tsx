import { StrategyCards } from "@/components/strategy/StrategyCards";
import { AdvancedParams } from "@/components/strategy/AdvancedParams";

export function StrategyPage() {
  return (
    <section className="mx-auto max-w-2xl space-y-6">
      <h2 className="text-2xl font-semibold leading-apple-headline tracking-apple-tight">
        切换策略
      </h2>
      <StrategyCards />
      <AdvancedParams />
    </section>
  );
}
