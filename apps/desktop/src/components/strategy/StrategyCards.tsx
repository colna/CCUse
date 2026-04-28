import { useCallback, useEffect, useState } from "react";
import { ArrowUpDown, Gauge, Coins, RotateCcw, Brain } from "lucide-react";
import { useTranslation } from "react-i18next";

import { cn } from "@/lib/utils";
import {
  getStrategy,
  setStrategy,
  type SwitchStrategy,
  type StrategyResponse,
} from "@/lib/tauri";

interface StrategyOption {
  id: SwitchStrategy;
  labelKey: string;
  descKey: string;
  icon: React.ElementType;
}

const STRATEGIES: StrategyOption[] = [
  {
    id: "priority",
    labelKey: "strategy_priority",
    descKey: "strategy_priority_desc",
    icon: ArrowUpDown,
  },
  {
    id: "fastest",
    labelKey: "strategy_fastest",
    descKey: "strategy_fastest_desc",
    icon: Gauge,
  },
  {
    id: "cost",
    labelKey: "strategy_cost",
    descKey: "strategy_cost_desc",
    icon: Coins,
  },
  {
    id: "load_balance",
    labelKey: "strategy_load_balance",
    descKey: "strategy_load_balance_desc",
    icon: RotateCcw,
  },
  {
    id: "smart",
    labelKey: "strategy_smart",
    descKey: "strategy_smart_desc",
    icon: Brain,
  },
];

export function StrategyCards() {
  const { t } = useTranslation("strategy");
  const [config, setConfig] = useState<StrategyResponse | null>(null);
  const [updating, setUpdating] = useState(false);

  useEffect(() => {
    getStrategy().then(setConfig).catch(console.error);
  }, []);

  const handleSelect = useCallback(
    async (strategy: SwitchStrategy) => {
      if (!config || strategy === config.strategy || updating) return;
      setUpdating(true);
      try {
        await setStrategy(strategy);
        setConfig((prev) => (prev ? { ...prev, strategy } : prev));
      } catch (err) {
        console.error("Failed to set strategy:", err);
      } finally {
        setUpdating(false);
      }
    },
    [config, updating],
  );

  return (
    <div className="space-y-4">
      <header className="space-y-1">
        <h3 className="text-base font-semibold leading-apple-headline tracking-apple-tight">
          {t("title")}
        </h3>
        <p className="text-xs text-muted-foreground">{t("strategy_desc")}</p>
      </header>

      <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {STRATEGIES.map((s) => {
          const Icon = s.icon;
          const selected = config?.strategy === s.id;
          return (
            <button
              key={s.id}
              onClick={() => handleSelect(s.id)}
              disabled={updating}
              className={cn(
                "flex flex-col items-start gap-2 rounded-xl border p-4 text-left transition-all",
                selected
                  ? "border-primary bg-primary/5 ring-1 ring-primary/30"
                  : "border-border bg-card hover:border-primary/40 hover:bg-card/80",
                updating && "opacity-60",
              )}
            >
              <div className="flex items-center gap-2">
                <Icon
                  className={cn(
                    "size-4",
                    selected ? "text-primary" : "text-muted-foreground",
                  )}
                />
                <span className="text-sm font-medium">{t(s.labelKey)}</span>
                {selected && (
                  <span className="rounded-full bg-primary/10 px-2 py-0.5 text-[10px] font-medium text-primary">
                    {t("current_badge")}
                  </span>
                )}
              </div>
              <p className="text-xs leading-relaxed text-muted-foreground">
                {t(s.descKey)}
              </p>
            </button>
          );
        })}
      </div>
    </div>
  );
}
