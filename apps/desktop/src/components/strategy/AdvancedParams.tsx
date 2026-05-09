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

type WeightKey = keyof SmartWeights;

const TOTAL_WEIGHT = 100;
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

function distributeWeight(
  keys: WeightKey[],
  source: SmartWeights,
  targetTotal: number,
): Record<WeightKey, number> {
  const target = clampWeight(targetTotal);
  const result = Object.fromEntries(
    WEIGHT_KEYS.map((key) => [key, 0]),
  ) as Record<WeightKey, number>;

  if (keys.length === 0 || target === 0) {
    return result;
  }

  const sourceTotal = keys.reduce(
    (sum, key) => sum + clampWeight(source[key]),
    0,
  );

  if (sourceTotal === 0) {
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

  if (totalWeight(cleaned) === TOTAL_WEIGHT) {
    return cleaned;
  }

  const distributed = distributeWeight(WEIGHT_KEYS, cleaned, TOTAL_WEIGHT);
  return {
    health: distributed.health,
    response_time: distributed.response_time,
    cost: distributed.cost,
    priority: distributed.priority,
  };
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

  return {
    health: changedKey === "health" ? nextValue : distributed.health,
    response_time:
      changedKey === "response_time" ? nextValue : distributed.response_time,
    cost: changedKey === "cost" ? nextValue : distributed.cost,
    priority: changedKey === "priority" ? nextValue : distributed.priority,
  };
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
      setTimeout(() => setSaved(false), 2000);
    } catch (err) {
      console.error("Failed to update params:", err);
    } finally {
      setSaving(false);
    }
  }, [maxRetries, weights]);

  const handleWeightChange = useCallback(
    (key: keyof SmartWeights, value: number) => {
      setWeights((prev) => rebalanceWeights(prev, key, value));
    },
    [],
  );

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
          <WeightSlider
            label={t("weight_health")}
            value={weights.health}
            onChange={(v) => handleWeightChange("health", v)}
          />
          <WeightSlider
            label={t("weight_response_time")}
            value={weights.response_time}
            onChange={(v) => handleWeightChange("response_time", v)}
          />
          <WeightSlider
            label={t("weight_cost")}
            value={weights.cost}
            onChange={(v) => handleWeightChange("cost", v)}
          />
          <WeightSlider
            label={t("weight_priority")}
            value={weights.priority}
            onChange={(v) => handleWeightChange("priority", v)}
          />
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
