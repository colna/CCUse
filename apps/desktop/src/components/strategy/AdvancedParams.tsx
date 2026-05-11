import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import {
  getStrategy,
  updateStrategyParams,
  type SmartWeights,
  type StrategyResponse,
} from "@/lib/tauri";

/**
 * "高级参数"面板：最大重试 + 智能策略的 4 维权重。
 *
 * 智能权重的约束是"总和恒等于 100"。用户拖动任一滑块时，剩余三个会
 * 按当前比例自动重新分配；这样既保证总和守恒、又不需要用户手动平衡。
 * 算法上分两步：
 *   1. clamp 每个值到 [0, 100]、向下取整；
 *   2. 把剩余预算按"小数余项最大优先"分配，最后总和精确为 100。
 *
 * 测试钉住了 (40,30,20,10) 起步、滑健康度到 50 → (50,25,17,8) 的具体
 * 数字，所以这套算法不能随意改动。
 */

type WeightKey = keyof SmartWeights;

const TOTAL_WEIGHT = 100;
const SAVED_HINT_MS = 2000;
const WEIGHT_KEYS: WeightKey[] = [
  "health",
  "response_time",
  "cost",
  "priority",
];

function clampWeight(value: number): number {
  return Math.min(TOTAL_WEIGHT, Math.max(0, Math.round(value)));
}

function totalWeight(weights: SmartWeights): number {
  return WEIGHT_KEYS.reduce((sum, key) => sum + weights[key], 0);
}

/** 把 `target` 总量按 `source` 的现有比例分到 `keys` 列出的字段；
 * 其余字段记 0。源全 0 时改为均分，保证不会出现"全黑屏"的极端态。 */
function distributeWeight(
  keys: WeightKey[],
  source: SmartWeights,
  targetTotal: number,
): Record<WeightKey, number> {
  const target = clampWeight(targetTotal);
  const result = Object.fromEntries(
    WEIGHT_KEYS.map((key) => [key, 0]),
  ) as Record<WeightKey, number>;

  if (keys.length === 0 || target === 0) return result;

  const sourceTotal = keys.reduce(
    (sum, key) => sum + clampWeight(source[key]),
    0,
  );

  if (sourceTotal === 0) {
    // 兜底：源全为 0，没有比例可借，改为均分；剩余 1..k-1 按顺序补齐
    // 头部，保证总和精确等于 target。
    const base = Math.floor(target / keys.length);
    let remainder = target - base * keys.length;
    for (const key of keys) {
      result[key] = base + (remainder > 0 ? 1 : 0);
      remainder = Math.max(0, remainder - 1);
    }
    return result;
  }

  const scaled = keys.map((key) => {
    const raw = (clampWeight(source[key]) / sourceTotal) * target;
    const base = Math.floor(raw);
    return { key, base, fraction: raw - base };
  });

  // "余项最大优先"分配，保证整数化后的总和精确等于 target。
  let remainder = target - scaled.reduce((sum, item) => sum + item.base, 0);
  const byLargestFraction = [...scaled].sort((a, b) => b.fraction - a.fraction);
  for (const item of byLargestFraction) {
    result[item.key] = item.base + (remainder > 0 ? 1 : 0);
    remainder = Math.max(0, remainder - 1);
  }

  return result;
}

function normalizeWeights(weights: SmartWeights): SmartWeights {
  const cleaned: SmartWeights = {
    health: clampWeight(weights.health),
    response_time: clampWeight(weights.response_time),
    cost: clampWeight(weights.cost),
    priority: clampWeight(weights.priority),
  };

  if (totalWeight(cleaned) === TOTAL_WEIGHT) return cleaned;

  const distributed = distributeWeight(WEIGHT_KEYS, cleaned, TOTAL_WEIGHT);
  return { ...cleaned, ...distributed };
}

function rebalanceWeights(
  weights: SmartWeights,
  changedKey: WeightKey,
  value: number,
): SmartWeights {
  const nextValue = clampWeight(value);
  const otherKeys = WEIGHT_KEYS.filter((key) => key !== changedKey);
  const distributed = distributeWeight(
    otherKeys,
    weights,
    TOTAL_WEIGHT - nextValue,
  );
  return { ...distributed, [changedKey]: nextValue };
}

export function AdvancedParams() {
  const { t } = useTranslation("strategy");
  const { t: tc } = useTranslation("common");
  const [config, setConfig] = useState<StrategyResponse | null>(null);
  const [maxRetries, setMaxRetries] = useState("3");
  const [weights, setWeights] = useState<SmartWeights>({
    health: 40,
    response_time: 30,
    cost: 20,
    priority: 10,
  });
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    getStrategy()
      .then((c) => {
        setConfig(c);
        setMaxRetries(String(c.max_retries));
        setWeights(normalizeWeights(c.smart_weights));
      })
      .catch(console.error);
  }, []);

  const handleSave = useCallback(async () => {
    setSaving(true);
    setSaved(false);
    try {
      await updateStrategyParams({
        max_retries: Number(maxRetries),
        smart_weights: weights,
      });
      setSaved(true);
      setTimeout(() => setSaved(false), SAVED_HINT_MS);
    } catch (err) {
      console.error("Failed to update params:", err);
    } finally {
      setSaving(false);
    }
  }, [maxRetries, weights]);

  const handleWeightChange = useCallback((key: WeightKey, value: number) => {
    setWeights((prev) => rebalanceWeights(prev, key, value));
  }, []);

  if (!config) return null;

  const smartWeightTotal = totalWeight(weights);
  const smartWeightIsValid = smartWeightTotal === TOTAL_WEIGHT;

  return (
    <div className="space-y-5 rounded-2xl border border-[var(--app-border-secondary)] bg-[var(--app-bg-container)] p-6">
      <header className="space-y-1">
        <h3 className="text-base font-semibold leading-apple-headline tracking-apple-tight">
          {t("advanced_title")}
        </h3>
        <p className="text-xs text-muted-foreground">{t("advanced_desc")}</p>
      </header>

      <div className="space-y-1.5">
        <label
          htmlFor="max-retries"
          className="block text-xs uppercase tracking-[0.18em] text-muted-foreground"
        >
          {t("max_retries_label")}
        </label>
        <input
          id="max-retries"
          type="number"
          min={0}
          max={10}
          value={maxRetries}
          onChange={(e) => setMaxRetries(e.target.value)}
          className="w-full rounded-md border border-[var(--app-border)] bg-[var(--app-bg-container)] px-3 py-2 text-sm outline-none focus-visible:border-[var(--app-primary)]"
        />
        <p className="text-xs text-muted-foreground">{t("max_retries_hint")}</p>
      </div>

      {config.strategy === "smart" && (
        <div className="space-y-4">
          <p className="text-xs uppercase tracking-[0.18em] text-muted-foreground">
            {t("smart_weights_label")}
          </p>
          {(
            [
              ["health", "weight_health"],
              ["response_time", "weight_response_time"],
              ["cost", "weight_cost"],
              ["priority", "weight_priority"],
            ] as const
          ).map(([key, labelKey]) => (
            <WeightSlider
              key={key}
              label={t(labelKey)}
              value={weights[key]}
              onChange={(v) => handleWeightChange(key, v)}
            />
          ))}
          <p className="text-xs text-muted-foreground">
            {t("weight_total", {
              total: smartWeightTotal,
              target: TOTAL_WEIGHT,
            })}
          </p>
          <p
            aria-live="polite"
            className={cn(
              "text-xs",
              smartWeightIsValid ? "text-primary" : "text-destructive",
            )}
          >
            {t("weight_total_status", {
              total: smartWeightTotal,
              target: TOTAL_WEIGHT,
            })}
          </p>
        </div>
      )}

      <footer className="flex items-center justify-end gap-3">
        {saved && <span className="text-xs text-primary">{tc("saved")}</span>}
        <Button
          type="primary"
          onClick={handleSave}
          disabled={saving || !smartWeightIsValid}
        >
          {saving ? tc("saving") : tc("save")}
        </Button>
      </footer>
    </div>
  );
}

interface WeightSliderProps {
  label: string;
  value: number;
  onChange: (value: number) => void;
}

function WeightSlider({ label, value, onChange }: WeightSliderProps) {
  return (
    <div className="flex items-center gap-3">
      <span className="w-16 text-xs text-muted-foreground">{label}</span>
      <input
        type="range"
        aria-label={label}
        min={0}
        max={100}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="h-1.5 flex-1 cursor-pointer appearance-none rounded-full bg-muted accent-primary"
      />
      <span className="w-8 text-right text-xs tabular-nums text-muted-foreground">
        {value}
      </span>
    </div>
  );
}
