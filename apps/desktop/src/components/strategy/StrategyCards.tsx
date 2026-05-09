import { useCallback, useEffect, useState } from "react";
import {
  SwapOutlined,
  ThunderboltOutlined,
  DollarOutlined,
  RetweetOutlined,
  RobotOutlined,
} from "@ant-design/icons";
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
    icon: SwapOutlined,
  },
  {
    id: "fastest",
    labelKey: "strategy_fastest",
    descKey: "strategy_fastest_desc",
    icon: ThunderboltOutlined,
  },
  {
    id: "cost",
    labelKey: "strategy_cost",
    descKey: "strategy_cost_desc",
    icon: DollarOutlined,
  },
  {
    id: "load_balance",
    labelKey: "strategy_load_balance",
    descKey: "strategy_load_balance_desc",
    icon: RetweetOutlined,
  },
  {
    id: "smart",
    labelKey: "strategy_smart",
    descKey: "strategy_smart_desc",
    icon: RobotOutlined,
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

      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {STRATEGIES.map((s) => {
          const Icon = s.icon;
          const selected = config?.strategy === s.id;
          return (
            <button
              key={s.id}
              onClick={() => handleSelect(s.id)}
              disabled={updating}
              className={cn(
                "flex flex-col items-start gap-2 rounded-2xl border p-5 text-left transition-all",
                selected
                  ? "border-[var(--app-primary)] bg-[var(--app-primary-bg)]"
                  : "hover:border-[var(--app-primary)]/40 border-[var(--app-border-secondary)] bg-[var(--app-bg-container)]",
                updating && "opacity-60",
              )}
            >
              <div className="flex w-full items-center gap-2">
                <Icon
                  className={cn(
                    "text-base",
                    selected
                      ? "text-[var(--app-primary)]"
                      : "text-muted-foreground",
                  )}
                />
                <span className="text-sm font-medium">{t(s.labelKey)}</span>
                {selected && (
                  <span className="bg-[var(--app-primary-bg))] ml-auto rounded-full px-2 py-0.5 text-[10px] font-medium text-[var(--app-primary)]">
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
