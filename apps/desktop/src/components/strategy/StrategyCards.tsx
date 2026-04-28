import { useCallback, useEffect, useState } from "react";
import {
  ArrowUpDown,
  Gauge,
  Coins,
  RotateCcw,
  Brain,
} from "lucide-react";

import { cn } from "@/lib/utils";
import {
  getStrategy,
  setStrategy,
  type SwitchStrategy,
  type StrategyResponse,
} from "@/lib/tauri";

interface StrategyOption {
  id: SwitchStrategy;
  label: string;
  description: string;
  icon: React.ElementType;
}

const STRATEGIES: StrategyOption[] = [
  {
    id: "priority",
    label: "优先级",
    description: "按优先级顺序选择供应商（数值越小越优先）",
    icon: ArrowUpDown,
  },
  {
    id: "fastest",
    label: "最快响应",
    description: "自动选择响应时间最短的供应商",
    icon: Gauge,
  },
  {
    id: "cost",
    label: "成本优先",
    description: "选择每 token 成本最低的供应商",
    icon: Coins,
  },
  {
    id: "load_balance",
    label: "负载均衡",
    description: "轮询所有可用供应商，平均分配请求",
    icon: RotateCcw,
  },
  {
    id: "smart",
    label: "智能策略",
    description: "综合健康度、响应速度、成本和优先级四维加权",
    icon: Brain,
  },
];

export function StrategyCards() {
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
          切换策略
        </h3>
        <p className="text-xs text-muted-foreground">
          选择供应商自动切换的决策方式
        </p>
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
                <span className="text-sm font-medium">{s.label}</span>
                {selected && (
                  <span className="rounded-full bg-primary/10 px-2 py-0.5 text-[10px] font-medium text-primary">
                    当前
                  </span>
                )}
              </div>
              <p className="text-xs leading-relaxed text-muted-foreground">
                {s.description}
              </p>
            </button>
          );
        })}
      </div>
    </div>
  );
}
