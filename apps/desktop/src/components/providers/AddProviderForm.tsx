import { useCallback, useState } from "react";
import { ChevronDown, ChevronRight, Loader2, Plug } from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import {
  addProvider,
  testProviderConnection,
  type ProviderInput,
} from "@/lib/tauri";

// ─── Provider type definitions ───────────────────────────────

type ProviderKind = ProviderInput["kind"];

interface ProviderTypeOption {
  kind: ProviderKind;
  label: string;
  defaultBaseUrl: string;
  requiresBaseUrl: boolean;
}

const PROVIDER_TYPES: ProviderTypeOption[] = [
  {
    kind: "openai",
    label: "OpenAI",
    defaultBaseUrl: "https://api.openai.com",
    requiresBaseUrl: false,
  },
  {
    kind: "anthropic",
    label: "Anthropic",
    defaultBaseUrl: "https://api.anthropic.com",
    requiresBaseUrl: false,
  },
  {
    kind: "gemini",
    label: "Gemini",
    defaultBaseUrl: "https://generativelanguage.googleapis.com",
    requiresBaseUrl: false,
  },
  { kind: "relay", label: "Relay", defaultBaseUrl: "", requiresBaseUrl: true },
  {
    kind: "custom",
    label: "Custom",
    defaultBaseUrl: "",
    requiresBaseUrl: true,
  },
];

// ─── Form values & validation ────────────────────────────────

interface FormValues {
  kind: ProviderKind;
  name: string;
  base_url: string;
  api_key: string;
  priority: string;
  enabled: boolean;
  monthly_quota: string;
  rate_limit_rpm: string;
  cost_per_1k_tokens: string;
}

interface FieldErrors {
  name?: string;
  base_url?: string;
  api_key?: string;
  priority?: string;
  monthly_quota?: string;
  rate_limit_rpm?: string;
  cost_per_1k_tokens?: string;
}

const INITIAL_VALUES: FormValues = {
  kind: "openai",
  name: "",
  base_url: "https://api.openai.com",
  api_key: "",
  priority: "100",
  enabled: true,
  monthly_quota: "",
  rate_limit_rpm: "",
  cost_per_1k_tokens: "",
};

const PRIORITY_MIN = 1;
const PRIORITY_MAX = 1000;

function validate(values: FormValues): FieldErrors {
  const errors: FieldErrors = {};
  if (!values.name.trim()) errors.name = "Name is required";

  const trimmedUrl = values.base_url.trim();
  const typeOption = PROVIDER_TYPES.find((t) => t.kind === values.kind);

  if (!trimmedUrl) {
    if (typeOption?.requiresBaseUrl) {
      errors.base_url = "Base URL is required for this provider type";
    }
  } else {
    try {
      const url = new URL(trimmedUrl);
      if (url.protocol !== "https:" && url.protocol !== "http:") {
        errors.base_url = "Base URL must start with http:// or https://";
      }
    } catch {
      errors.base_url = "Invalid Base URL format";
    }
  }

  if (!values.api_key.trim()) errors.api_key = "API Key is required";

  const priority = Number(values.priority);
  if (!Number.isInteger(priority)) {
    errors.priority = "Priority must be an integer";
  } else if (priority < PRIORITY_MIN || priority > PRIORITY_MAX) {
    errors.priority = `Priority must be ${PRIORITY_MIN}--${PRIORITY_MAX}`;
  }

  if (values.monthly_quota.trim()) {
    const v = Number(values.monthly_quota);
    if (isNaN(v) || v < 0)
      errors.monthly_quota = "Must be a non-negative number";
  }
  if (values.rate_limit_rpm.trim()) {
    const v = Number(values.rate_limit_rpm);
    if (!Number.isInteger(v) || v < 0)
      errors.rate_limit_rpm = "Must be a non-negative integer";
  }
  if (values.cost_per_1k_tokens.trim()) {
    const v = Number(values.cost_per_1k_tokens);
    if (isNaN(v) || v < 0)
      errors.cost_per_1k_tokens = "Must be a non-negative number";
  }

  return errors;
}

// ─── Component ───────────────────────────────────────────────

interface AddProviderFormProps {
  onAdded?: (id: string) => void;
}

export function AddProviderForm({ onAdded }: AddProviderFormProps) {
  const [values, setValues] = useState<FormValues>(INITIAL_VALUES);
  const [errors, setErrors] = useState<FieldErrors>({});
  const [submitting, setSubmitting] = useState(false);
  const [serverError, setServerError] = useState<string | null>(null);
  const [successId, setSuccessId] = useState<string | null>(null);
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [testResult, setTestResult] = useState<{ latency: number } | null>(
    null,
  );
  const [testError, setTestError] = useState<string | null>(null);
  const [testing, setTesting] = useState(false);

  const handleKindChange = useCallback((kind: ProviderKind) => {
    const typeOption = PROVIDER_TYPES.find((t) => t.kind === kind);
    setValues((s) => ({
      ...s,
      kind,
      base_url: typeOption?.defaultBaseUrl ?? "",
    }));
    setTestResult(null);
    setTestError(null);
  }, []);

  const handleTestConnection = useCallback(async () => {
    if (!successId) return;
    setTesting(true);
    setTestResult(null);
    setTestError(null);
    try {
      const latencyMs = await testProviderConnection(successId);
      setTestResult({ latency: latencyMs });
    } catch (err: unknown) {
      setTestError(err instanceof Error ? err.message : String(err));
    } finally {
      setTesting(false);
    }
  }, [successId]);

  const handleSubmit = useCallback(
    async (event: React.FormEvent<HTMLFormElement>) => {
      event.preventDefault();
      setSuccessId(null);
      setServerError(null);
      setTestResult(null);
      setTestError(null);
      const fieldErrors = validate(values);
      setErrors(fieldErrors);
      if (Object.keys(fieldErrors).length > 0) return;

      const input: ProviderInput = {
        name: values.name.trim(),
        kind: values.kind,
        base_url: values.base_url.trim().replace(/\/$/, ""),
        api_key: values.api_key.trim(),
        priority: Number(values.priority),
        enabled: values.enabled,
        monthly_quota: values.monthly_quota.trim()
          ? Number(values.monthly_quota)
          : null,
        rate_limit_rpm: values.rate_limit_rpm.trim()
          ? Number(values.rate_limit_rpm)
          : null,
        cost_per_1k_tokens: values.cost_per_1k_tokens.trim()
          ? Number(values.cost_per_1k_tokens)
          : null,
      };
      setSubmitting(true);
      try {
        const provider = await addProvider(input);
        setSuccessId(provider.id);
        onAdded?.(provider.id);
      } catch (err: unknown) {
        const message = err instanceof Error ? err.message : String(err);
        setServerError(message);
      } finally {
        setSubmitting(false);
      }
    },
    [values, onAdded],
  );

  const handleReset = useCallback(() => {
    setValues(INITIAL_VALUES);
    setErrors({});
    setServerError(null);
    setSuccessId(null);
    setTestResult(null);
    setTestError(null);
  }, []);

  return (
    <form
      noValidate
      onSubmit={handleSubmit}
      aria-label="Add provider"
      className="space-y-5 rounded-2xl border border-border bg-card p-6 shadow-apple-card"
    >
      <header className="space-y-1">
        <h3 className="text-base font-semibold leading-apple-headline tracking-apple-tight">
          Add Provider
        </h3>
        <p className="text-xs text-muted-foreground">
          Select provider type, fill in details. Lower priority number = higher
          preference.
        </p>
      </header>

      {/* Provider type selector */}
      <div className="space-y-1.5">
        <span className="text-xs uppercase tracking-[0.18em] text-muted-foreground">
          Provider Type
        </span>
        <div className="flex flex-wrap gap-2">
          {PROVIDER_TYPES.map((opt) => (
            <button
              key={opt.kind}
              type="button"
              onClick={() => handleKindChange(opt.kind)}
              className={cn(
                "rounded-lg border px-3 py-1.5 text-sm font-medium transition-colors",
                values.kind === opt.kind
                  ? "border-primary bg-primary/10 text-primary"
                  : "border-border bg-background text-muted-foreground hover:border-primary/40 hover:text-foreground",
              )}
            >
              {opt.label}
            </button>
          ))}
        </div>
      </div>

      <Field
        id="provider-name"
        label="Name"
        placeholder="e.g. Work OpenAI"
        value={values.name}
        onChange={(v) => setValues((s) => ({ ...s, name: v }))}
        error={errors.name}
      />

      <Field
        id="provider-base-url"
        label="Base URL"
        placeholder={
          PROVIDER_TYPES.find((t) => t.kind === values.kind)?.defaultBaseUrl ||
          "https://..."
        }
        value={values.base_url}
        onChange={(v) => setValues((s) => ({ ...s, base_url: v }))}
        error={errors.base_url}
      />

      <Field
        id="provider-api-key"
        label="API Key"
        type="password"
        placeholder="sk-..."
        value={values.api_key}
        onChange={(v) => setValues((s) => ({ ...s, api_key: v }))}
        error={errors.api_key}
      />

      <Field
        id="provider-priority"
        label="Priority"
        inputMode="numeric"
        placeholder="100"
        value={values.priority}
        onChange={(v) => setValues((s) => ({ ...s, priority: v }))}
        error={errors.priority}
        hint={`Lower number = higher priority (${PRIORITY_MIN}--${PRIORITY_MAX})`}
      />

      <label className="flex items-center gap-2 text-sm">
        <input
          type="checkbox"
          checked={values.enabled}
          onChange={(e) =>
            setValues((s) => ({ ...s, enabled: e.target.checked }))
          }
          className="size-4 rounded border-border accent-primary"
        />
        <span>Enable this provider (participates in switching)</span>
      </label>

      {/* Advanced section (collapsible) */}
      <div className="rounded-xl border border-border">
        <button
          type="button"
          onClick={() => setAdvancedOpen((o) => !o)}
          className="flex w-full items-center gap-2 px-4 py-2.5 text-sm font-medium text-muted-foreground hover:text-foreground"
        >
          {advancedOpen ? (
            <ChevronDown className="size-4" />
          ) : (
            <ChevronRight className="size-4" />
          )}
          Advanced (Quota / Rate Limit / Cost)
        </button>
        {advancedOpen && (
          <div className="space-y-4 border-t border-border px-4 py-4">
            <Field
              id="provider-monthly-quota"
              label="Monthly Quota ($)"
              inputMode="decimal"
              placeholder="e.g. 100"
              value={values.monthly_quota}
              onChange={(v) => setValues((s) => ({ ...s, monthly_quota: v }))}
              error={errors.monthly_quota}
              hint="Monthly spending limit in USD (optional)"
            />
            <Field
              id="provider-rate-limit-rpm"
              label="Rate Limit (RPM)"
              inputMode="numeric"
              placeholder="e.g. 60"
              value={values.rate_limit_rpm}
              onChange={(v) => setValues((s) => ({ ...s, rate_limit_rpm: v }))}
              error={errors.rate_limit_rpm}
              hint="Requests per minute limit (optional)"
            />
            <Field
              id="provider-cost-per-1k"
              label="Cost per 1K tokens ($)"
              inputMode="decimal"
              placeholder="e.g. 0.03"
              value={values.cost_per_1k_tokens}
              onChange={(v) =>
                setValues((s) => ({ ...s, cost_per_1k_tokens: v }))
              }
              error={errors.cost_per_1k_tokens}
              hint="Cost per 1,000 tokens in USD (optional)"
            />
          </div>
        )}
      </div>

      <footer className="space-y-3">
        <div className="flex items-center justify-between gap-3">
          <p
            aria-live="polite"
            className={cn(
              "text-xs",
              serverError
                ? "text-destructive"
                : successId
                  ? "text-primary"
                  : "text-muted-foreground",
            )}
          >
            {serverError
              ? serverError
              : successId
                ? `Added (id: ${successId.slice(0, 8)}...)`
                : ""}
          </p>
          <div className="flex items-center gap-2">
            {successId && (
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={handleReset}
              >
                Add Another
              </Button>
            )}
            <Button type="submit" disabled={submitting || Boolean(successId)}>
              {submitting ? "Adding..." : "Add"}
            </Button>
          </div>
        </div>

        {/* Test Connection (only after successful add) */}
        {successId && (
          <div className="flex items-center gap-3 rounded-lg border border-border bg-muted/30 px-4 py-3">
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={handleTestConnection}
              disabled={testing}
            >
              {testing ? (
                <Loader2 className="mr-2 size-3.5 animate-spin" />
              ) : (
                <Plug className="mr-2 size-3.5" />
              )}
              Test Connection
            </Button>
            {testResult && (
              <span className="text-xs text-green-600">
                Connected ({testResult.latency}ms)
              </span>
            )}
            {testError && (
              <span className="text-xs text-destructive">
                Failed: {testError}
              </span>
            )}
          </div>
        )}
      </footer>
    </form>
  );
}

// ─── Shared Field component ──────────────────────────────────

interface FieldProps {
  id: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
  error?: string;
  type?: string;
  inputMode?: React.HTMLAttributes<HTMLInputElement>["inputMode"];
  placeholder?: string;
  hint?: string;
}

function Field({
  id,
  label,
  value,
  onChange,
  error,
  type = "text",
  inputMode,
  placeholder,
  hint,
}: FieldProps) {
  const errorId = `${id}-error`;
  const hintId = `${id}-hint`;
  return (
    <div className="space-y-1.5">
      <label
        htmlFor={id}
        className="text-xs uppercase tracking-[0.18em] text-muted-foreground"
      >
        {label}
      </label>
      <input
        id={id}
        type={type}
        inputMode={inputMode}
        value={value}
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value)}
        aria-invalid={Boolean(error)}
        aria-describedby={cn(error && errorId, hint && hintId) || undefined}
        className={cn(
          "w-full rounded-md border bg-background px-3 py-2 text-sm leading-snug tracking-apple-tight outline-none transition-colors",
          error
            ? "border-destructive/60 focus-visible:border-destructive"
            : "border-border focus-visible:border-primary",
        )}
      />
      {hint ? (
        <p id={hintId} className="text-xs text-muted-foreground">
          {hint}
        </p>
      ) : null}
      {error ? (
        <p id={errorId} className="text-xs text-destructive">
          {error}
        </p>
      ) : null}
    </div>
  );
}
