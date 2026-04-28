import { useTranslation } from "react-i18next";

import { StrategyCards } from "@/components/strategy/StrategyCards";
import { AdvancedParams } from "@/components/strategy/AdvancedParams";

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
