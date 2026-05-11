import { useCallback, useState } from "react";
import {
  CaretDownOutlined,
  CaretRightOutlined,
  ApiOutlined,
  CheckOutlined,
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

/**
 * "添加供应商"表单。
 *
 * 设计点：
 * - 所有字段保留为字符串原样，提交时统一 `trim` + 数值转换；这样
 *   `aria-invalid` / 错误提示能精确指向用户输入的位置，而不会被
 *   number 解析提前吞掉。
 * - 添加成功后，提交按钮锁死；同时露出一个"测试连接"按钮，避免用户
 *   重复提交相同表单。
 * - 高级选项默认折叠，默认值都是 null —— 后端把缺省视为"不限"。
 */

type TFn = (key: string, opts?: Record<string, string | number>) => string;

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

const PRIORITY_MIN = 1;
const PRIORITY_MAX = 1000;

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
    const opt = PROVIDER_KIND_OPTIONS.find((o) => o.kind === kind);
    setValues((s) => ({ ...s, kind, base_url: opt?.defaultBaseUrl ?? "" }));
    setTestResult(null);
    setTestError(null);
  }, []);

  const handleTestConnection = useCallback(async () => {
    if (!successId) return;
    setTesting(true);
    setTestResult(null);
    setTestError(null);
    try {
      setTestResult(await testProviderConnection(successId));
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

      setSubmitting(true);
      try {
        const provider = await addProvider(buildProviderInput(values));
        setSuccessId(provider.id);
        onAdded?.(provider.id);
      } catch (err: unknown) {
        setServerError(err instanceof Error ? err.message : String(err));
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
      className="space-y-5 rounded-2xl border border-[var(--app-border-secondary)] bg-[var(--app-bg-container)] p-6"
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

      <KindPicker
        value={values.kind}
        disabled={submitting}
        onChange={handleKindChange}
        t={t}
      />

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
          PROVIDER_KIND_OPTIONS.find((o) => o.kind === values.kind)
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

      <AdvancedSection
        open={advancedOpen}
        disabled={submitting}
        values={values}
        errors={errors}
        onToggle={() => setAdvancedOpen((o) => !o)}
        onChange={setValues}
        t={t}
      />

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
          <TestConnectionStrip
            testing={testing}
            testResult={testResult}
            testError={testError}
            onClick={handleTestConnection}
            t={t}
          />
        )}
      </footer>
    </form>
  );
}

// ─── 表单内部子组件 ───────────────────────────────────────────────────

interface KindPickerProps {
  value: ProviderKind;
  disabled: boolean;
  onChange: (kind: ProviderKind) => void;
  t: TFn;
}

function KindPicker({ value, disabled, onChange, t }: KindPickerProps) {
  return (
    <div className="space-y-2">
      <span className="text-xs uppercase tracking-[0.18em] text-muted-foreground">
        {t("provider_type")}
      </span>
      <div className="flex flex-wrap gap-2">
        {PROVIDER_KIND_OPTIONS.map((opt) => {
          const selected = value === opt.kind;
          return (
            <Button
              key={opt.kind}
              type={selected ? "primary" : "default"}
              disabled={disabled || !opt.supported}
              onClick={() => onChange(opt.kind)}
              size="middle"
              aria-pressed={selected}
              icon={
                selected ? (
                  <CheckOutlined aria-label="" role="presentation" />
                ) : undefined
              }
              style={
                selected
                  ? {
                      boxShadow:
                        "0 0 0 2px var(--app-primary-bg), 0 4px 12px rgba(0, 113, 227, 0.18)",
                    }
                  : undefined
              }
            >
              {opt.label}
            </Button>
          );
        })}
      </div>
    </div>
  );
}

interface AdvancedSectionProps {
  open: boolean;
  disabled: boolean;
  values: FormValues;
  errors: FieldErrors;
  onToggle: () => void;
  onChange: React.Dispatch<React.SetStateAction<FormValues>>;
  t: TFn;
}

function AdvancedSection({
  open,
  disabled,
  values,
  errors,
  onToggle,
  onChange,
  t,
}: AdvancedSectionProps) {
  return (
    <div className="rounded-xl border border-[var(--app-border-secondary)]">
      <button
        type="button"
        disabled={disabled}
        onClick={onToggle}
        className="flex w-full items-center gap-2 px-4 py-2.5 text-sm font-medium text-muted-foreground hover:text-foreground disabled:cursor-not-allowed disabled:opacity-60"
      >
        {open ? (
          <CaretDownOutlined className="text-xs" />
        ) : (
          <CaretRightOutlined className="text-xs" />
        )}
        {t("advanced_section")}
      </button>
      {open && (
        <div className="space-y-4 border-t border-[var(--app-border-secondary)] px-4 py-4">
          <Field
            id="provider-monthly-quota"
            label={t("field_monthly_quota")}
            inputMode="decimal"
            placeholder={t("field_monthly_quota_placeholder")}
            value={values.monthly_quota}
            onChange={(v) => onChange((s) => ({ ...s, monthly_quota: v }))}
            error={errors.monthly_quota}
            hint={t("field_monthly_quota_hint")}
            disabled={disabled}
          />
          <Field
            id="provider-rate-limit-rpm"
            label={t("field_rate_limit")}
            inputMode="numeric"
            placeholder={t("field_rate_limit_placeholder")}
            value={values.rate_limit_rpm}
            onChange={(v) => onChange((s) => ({ ...s, rate_limit_rpm: v }))}
            error={errors.rate_limit_rpm}
            hint={t("field_rate_limit_hint")}
            disabled={disabled}
          />
          <Field
            id="provider-cost-per-1k"
            label={t("field_cost_per_1k")}
            inputMode="decimal"
            placeholder={t("field_cost_per_1k_placeholder")}
            value={values.cost_per_1k_tokens}
            onChange={(v) => onChange((s) => ({ ...s, cost_per_1k_tokens: v }))}
            error={errors.cost_per_1k_tokens}
            hint={t("field_cost_per_1k_hint")}
            disabled={disabled}
          />
        </div>
      )}
    </div>
  );
}

interface TestConnectionStripProps {
  testing: boolean;
  testResult: StreamCheckResult | null;
  testError: string | null;
  onClick: () => void;
  t: TFn;
}

function TestConnectionStrip({
  testing,
  testResult,
  testError,
  onClick,
  t,
}: TestConnectionStripProps) {
  return (
    <div className="flex items-center gap-3 rounded-lg border border-[var(--app-border-secondary)] bg-[var(--app-bg-subtle)] px-4 py-3">
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
        onClick={onClick}
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
          {testResult.http_status != null && (
            <span className="ml-2">HTTP {testResult.http_status}</span>
          )}
        </div>
      )}
      {testError && (
        <span className="text-xs text-destructive">
          {t("test_failed", { error: testError })}
        </span>
      )}
    </div>
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
  const commonProps = {
    id,
    value,
    placeholder,
    disabled,
    "aria-invalid": Boolean(error),
    "aria-describedby": describedBy,
    status: error ? ("error" as const) : undefined,
    onChange: (e: React.ChangeEvent<HTMLInputElement>) =>
      onChange(e.target.value),
  };

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

// ─── 校验 + 提交体构造 ────────────────────────────────────────────────

function validate(values: FormValues, t: TFn): FieldErrors {
  const errors: FieldErrors = {};
  if (!values.name.trim()) errors.name = t("validation_name_required");

  const trimmedUrl = values.base_url.trim();
  const typeOption = PROVIDER_KIND_OPTIONS.find((o) => o.kind === values.kind);
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

  // 三个高级字段都是"留空视为不限"；只在用户实际填了内容时校验。
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

function optionalNumber(raw: string): number | null {
  return raw.trim() ? Number(raw) : null;
}

function buildProviderInput(values: FormValues): ProviderInput {
  return {
    name: values.name.trim(),
    kind: values.kind,
    // 末尾斜杠会让 Rust 端拼接路径时出现 `//` —— 后端实际能容错，但
    // 保持入库的形式整洁。
    base_url: values.base_url.trim().replace(/\/$/, ""),
    api_key: values.api_key.trim(),
    priority: Number(values.priority),
    enabled: values.enabled,
    monthly_quota: optionalNumber(values.monthly_quota),
    rate_limit_rpm: optionalNumber(values.rate_limit_rpm),
    cost_per_1k_tokens: optionalNumber(values.cost_per_1k_tokens),
  };
}
