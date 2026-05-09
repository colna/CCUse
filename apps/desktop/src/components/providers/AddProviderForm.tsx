import { useCallback, useState } from "react";
import {
  CaretDownOutlined,
  CaretRightOutlined,
  ApiOutlined,
  LoadingOutlined,
} from "@ant-design/icons";
import { Input } from "antd";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { PROVIDER_KIND_OPTIONS, type ProviderKind } from "@/lib/providerKinds";
import { cn } from "@/lib/utils";
import {
  addProvider,
  testProviderConnection,
  type ProviderInput,
  type StreamCheckResult,
} from "@/lib/tauri";

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

function validate(
  values: FormValues,
  t: (key: string, opts?: Record<string, string | number>) => string,
): FieldErrors {
  const errors: FieldErrors = {};
  if (!values.name.trim()) errors.name = t("validation_name_required");

  const trimmedUrl = values.base_url.trim();
  const typeOption = PROVIDER_KIND_OPTIONS.find(
    (tp) => tp.kind === values.kind,
  );

  if (!trimmedUrl) {
    if (typeOption?.requiresBaseUrl) {
      errors.base_url = t("validation_base_url_required");
    }
  } else {
    try {
      const url = new URL(trimmedUrl);
      if (url.protocol !== "https:" && url.protocol !== "http:") {
        errors.base_url = t("validation_base_url_protocol");
      }
    } catch {
      errors.base_url = t("validation_base_url_invalid");
    }
  }

  if (!values.api_key.trim()) errors.api_key = t("validation_api_key_required");

  const priority = Number(values.priority);
  if (!Number.isInteger(priority)) {
    errors.priority = t("validation_priority_integer");
  } else if (priority < PRIORITY_MIN || priority > PRIORITY_MAX) {
    errors.priority = t("validation_priority_range", {
      min: PRIORITY_MIN,
      max: PRIORITY_MAX,
    });
  }

  if (values.monthly_quota.trim()) {
    const v = Number(values.monthly_quota);
    if (isNaN(v) || v < 0)
      errors.monthly_quota = t("validation_non_negative_number");
  }
  if (values.rate_limit_rpm.trim()) {
    const v = Number(values.rate_limit_rpm);
    if (!Number.isInteger(v) || v < 0)
      errors.rate_limit_rpm = t("validation_non_negative_integer");
  }
  if (values.cost_per_1k_tokens.trim()) {
    const v = Number(values.cost_per_1k_tokens);
    if (isNaN(v) || v < 0)
      errors.cost_per_1k_tokens = t("validation_non_negative_number");
  }

  return errors;
}

interface AddProviderFormProps {
  onAdded?: (id: string) => void;
}

export function AddProviderForm({ onAdded }: AddProviderFormProps) {
  const { t } = useTranslation("providers");
  const [values, setValues] = useState<FormValues>(INITIAL_VALUES);
  const [errors, setErrors] = useState<FieldErrors>({});
  const [submitting, setSubmitting] = useState(false);
  const [serverError, setServerError] = useState<string | null>(null);
  const [successId, setSuccessId] = useState<string | null>(null);
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [testResult, setTestResult] = useState<StreamCheckResult | null>(null);
  const [testError, setTestError] = useState<string | null>(null);
  const [testing, setTesting] = useState(false);

  const handleKindChange = useCallback((kind: ProviderKind) => {
    const typeOption = PROVIDER_KIND_OPTIONS.find((tp) => tp.kind === kind);
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
      const result = await testProviderConnection(successId);
      setTestResult(result);
    } catch (err: unknown) {
      setTestError(err instanceof Error ? err.message : String(err));
    } finally {
      setTesting(false);
    }
  }, [successId]);

  const handleSubmit = useCallback(
    async (event: React.FormEvent<HTMLFormElement>) => {
      event.preventDefault();
      if (submitting) return;
      setSuccessId(null);
      setServerError(null);
      setTestResult(null);
      setTestError(null);
      const fieldErrors = validate(values, t);
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
    [values, submitting, onAdded, t],
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
      aria-label={t("add_provider_aria")}
      aria-busy={submitting}
      className="space-y-5 rounded-2xl border border-[var(--ant-color-border-secondary,rgba(0,0,0,0.06))] bg-[var(--ant-color-bg-container,#fff)] p-6"
      style={{
        boxShadow:
          "var(--ant-box-shadow-secondary, 0 1px 2px rgba(0,0,0,0.04))",
      }}
    >
      <header className="space-y-1">
        <h3 className="text-base font-semibold leading-apple-headline tracking-apple-tight">
          {t("add_provider_title")}
        </h3>
        <p className="text-xs text-muted-foreground">
          {t("add_provider_desc")}
        </p>
      </header>

      <div className="space-y-2">
        <span className="text-xs uppercase tracking-[0.18em] text-muted-foreground">
          {t("provider_type")}
        </span>
        <div className="flex flex-wrap gap-2">
          {PROVIDER_KIND_OPTIONS.map((opt) => {
            const selected = values.kind === opt.kind;
            return (
              <Button
                key={opt.kind}
                type={selected ? "primary" : "default"}
                disabled={submitting || !opt.supported}
                onClick={() => handleKindChange(opt.kind)}
                size="middle"
              >
                {opt.label}
              </Button>
            );
          })}
        </div>
      </div>

      <Field
        id="provider-name"
        label={t("field_name")}
        placeholder={t("field_name_placeholder")}
        value={values.name}
        onChange={(v) => setValues((s) => ({ ...s, name: v }))}
        error={errors.name}
        disabled={submitting}
      />

      <Field
        id="provider-base-url"
        label={t("field_base_url")}
        placeholder={
          PROVIDER_KIND_OPTIONS.find((tp) => tp.kind === values.kind)
            ?.defaultBaseUrl || "https://..."
        }
        value={values.base_url}
        onChange={(v) => setValues((s) => ({ ...s, base_url: v }))}
        error={errors.base_url}
        disabled={submitting}
      />

      <Field
        id="provider-api-key"
        label={t("field_api_key")}
        type="password"
        placeholder={t("field_api_key_placeholder")}
        value={values.api_key}
        onChange={(v) => setValues((s) => ({ ...s, api_key: v }))}
        error={errors.api_key}
        disabled={submitting}
      />

      <Field
        id="provider-priority"
        label={t("field_priority")}
        inputMode="numeric"
        placeholder={t("field_priority_placeholder")}
        value={values.priority}
        onChange={(v) => setValues((s) => ({ ...s, priority: v }))}
        error={errors.priority}
        hint={t("field_priority_hint", {
          min: PRIORITY_MIN,
          max: PRIORITY_MAX,
        })}
        disabled={submitting}
      />

      <label className="flex items-center gap-2 text-sm">
        <input
          type="checkbox"
          checked={values.enabled}
          disabled={submitting}
          onChange={(e) =>
            setValues((s) => ({ ...s, enabled: e.target.checked }))
          }
          className="size-4 rounded border-border accent-primary"
        />
        <span>{t("enable_provider_label")}</span>
      </label>

      <div className="rounded-xl border border-[var(--ant-color-border-secondary,rgba(0,0,0,0.06))]">
        <button
          type="button"
          disabled={submitting}
          onClick={() => setAdvancedOpen((o) => !o)}
          className="flex w-full items-center gap-2 px-4 py-2.5 text-sm font-medium text-muted-foreground hover:text-foreground disabled:cursor-not-allowed disabled:opacity-60"
        >
          {advancedOpen ? (
            <CaretDownOutlined className="text-xs" />
          ) : (
            <CaretRightOutlined className="text-xs" />
          )}
          {t("advanced_section")}
        </button>
        {advancedOpen && (
          <div className="space-y-4 border-t border-[var(--ant-color-border-secondary,rgba(0,0,0,0.06))] px-4 py-4">
            <Field
              id="provider-monthly-quota"
              label={t("field_monthly_quota")}
              inputMode="decimal"
              placeholder={t("field_monthly_quota_placeholder")}
              value={values.monthly_quota}
              onChange={(v) => setValues((s) => ({ ...s, monthly_quota: v }))}
              error={errors.monthly_quota}
              hint={t("field_monthly_quota_hint")}
              disabled={submitting}
            />
            <Field
              id="provider-rate-limit-rpm"
              label={t("field_rate_limit")}
              inputMode="numeric"
              placeholder={t("field_rate_limit_placeholder")}
              value={values.rate_limit_rpm}
              onChange={(v) => setValues((s) => ({ ...s, rate_limit_rpm: v }))}
              error={errors.rate_limit_rpm}
              hint={t("field_rate_limit_hint")}
              disabled={submitting}
            />
            <Field
              id="provider-cost-per-1k"
              label={t("field_cost_per_1k")}
              inputMode="decimal"
              placeholder={t("field_cost_per_1k_placeholder")}
              value={values.cost_per_1k_tokens}
              onChange={(v) =>
                setValues((s) => ({ ...s, cost_per_1k_tokens: v }))
              }
              error={errors.cost_per_1k_tokens}
              hint={t("field_cost_per_1k_hint")}
              disabled={submitting}
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
                ? t("added_id", { id: successId.slice(0, 8) })
                : ""}
          </p>
          <div className="flex items-center gap-2">
            {successId && (
              <Button type="default" onClick={handleReset}>
                {t("add_another")}
              </Button>
            )}
            <Button
              type="primary"
              htmlType="submit"
              disabled={submitting || Boolean(successId)}
              icon={
                submitting ? (
                  <LoadingOutlined
                    className="animate-spin"
                    aria-label=""
                    role="presentation"
                  />
                ) : undefined
              }
            >
              {submitting ? t("adding") : t("add")}
            </Button>
          </div>
        </div>

        {successId && (
          <div className="flex items-center gap-3 rounded-lg border border-[var(--ant-color-border-secondary,rgba(0,0,0,0.06))] bg-[var(--ant-color-fill-quaternary,rgba(0,0,0,0.02))] px-4 py-3">
            <Button
              type="default"
              icon={
                testing ? (
                  <LoadingOutlined
                    className="animate-spin"
                    aria-label=""
                    role="presentation"
                  />
                ) : (
                  <ApiOutlined aria-label="" role="presentation" />
                )
              }
              onClick={handleTestConnection}
              disabled={testing}
            >
              {t("test_connection")}
            </Button>
            {testResult && (
              <div className="text-xs text-muted-foreground">
                <span
                  className={
                    testResult.success ? "text-green-600" : "text-destructive"
                  }
                >
                  {testResult.success
                    ? t("test_connected", {
                        latency: testResult.response_time_ms ?? 0,
                      })
                    : testResult.message}
                </span>
                <span className="ml-2">
                  {testResult.http_status != null
                    ? `HTTP ${testResult.http_status}`
                    : ""}
                </span>
              </div>
            )}
            {testError && (
              <span className="text-xs text-destructive">
                {t("test_failed", { error: testError })}
              </span>
            )}
          </div>
        )}
      </footer>
    </form>
  );
}

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
  disabled?: boolean;
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
  disabled,
}: FieldProps) {
  const errorId = `${id}-error`;
  const hintId = `${id}-hint`;
  const describedBy = cn(error && errorId, hint && hintId) || undefined;
  const status = error ? "error" : undefined;
  const commonProps = {
    id,
    value,
    placeholder,
    disabled,
    "aria-invalid": Boolean(error),
    "aria-describedby": describedBy,
    status,
    onChange: (e: React.ChangeEvent<HTMLInputElement>) =>
      onChange(e.target.value),
  } as const;

  return (
    <div className="space-y-1.5">
      <label
        htmlFor={id}
        className="block text-xs uppercase tracking-[0.18em] text-muted-foreground"
      >
        {label}
      </label>
      {type === "password" ? (
        <Input.Password {...commonProps} />
      ) : (
        <Input {...commonProps} inputMode={inputMode} />
      )}
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
