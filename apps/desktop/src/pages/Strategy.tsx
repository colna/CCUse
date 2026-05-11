import { useTranslation } from "react-i18next";

import { AdvancedParams } from "@/components/strategy/AdvancedParams";
import { StrategyCards } from "@/components/strategy/StrategyCards";

/** 策略页：顶部是 5 种策略卡片，下面是高级参数（重试 + 智能权重）。 */
export function StrategyPage() {
  const { t } = useTranslation("strategy");

  return (
    <section className="mx-auto max-w-2xl space-y-6">
      <h2 className="text-2xl font-semibold leading-apple-headline tracking-apple-tight">
        {t("page_title")}
      </h2>
      <StrategyCards />
      <AdvancedParams />
    </section>
  );
}
