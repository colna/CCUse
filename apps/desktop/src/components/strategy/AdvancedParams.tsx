import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import {
  getStrategy,
  updateStrategyParams,
  type SmartWeights,
  type StrategyResponse,
} from "@/lib/tauri";

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
        setWeights(c.smart_weights);
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
      setWeights((prev) => ({ ...prev, [key]: value }));
    },
    [],
  );

  if (!config) return null;

  return (
    <div className="space-y-5 rounded-2xl border border-border bg-card p-6 shadow-apple-card">
      <header className="space-y-1">
        <h3 className="text-base font-semibold leading-apple-headline tracking-apple-tight">
          {t("advanced_title")}
        </h3>
        <p className="text-xs text-muted-foreground">{t("advanced_desc")}</p>
      </header>

      <div className="space-y-1.5">
        <label
          htmlFor="max-retries"
          className="text-xs uppercase tracking-[0.18em] text-muted-foreground"
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
          className="w-full rounded-md border border-border bg-background px-3 py-2 text-sm outline-none focus-visible:border-primary"
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
              total:
                weights.health +
                weights.response_time +
                weights.cost +
                weights.priority,
            })}
          </p>
        </div>
      )}

      <footer className="flex items-center justify-end gap-3">
        {saved && <span className="text-xs text-primary">{tc("saved")}</span>}
        <Button onClick={handleSave} disabled={saving}>
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
