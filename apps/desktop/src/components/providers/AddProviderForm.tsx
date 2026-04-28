import { useCallback, useState } from "react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { addProvider, type ProviderInput } from "@/lib/tauri";

interface FieldErrors {
  name?: string;
  base_url?: string;
  api_key?: string;
  priority?: string;
}

interface AddProviderFormProps {
  /** Called with the persisted provider after a successful submit.
   * Parent can update its list optimistically. */
  onAdded?: (id: string) => void;
}

const INITIAL_VALUES = {
  name: "",
  base_url: "https://api.openai.com",
  api_key: "",
  priority: "100",
  enabled: true,
};

const PRIORITY_MIN = 1;
const PRIORITY_MAX = 1000;

function validate(values: typeof INITIAL_VALUES): FieldErrors {
  const errors: FieldErrors = {};
  if (!values.name.trim()) errors.name = "名称不能为空";

  const trimmedUrl = values.base_url.trim();
  if (!trimmedUrl) {
    errors.base_url = "Base URL 不能为空";
  } else {
    try {
      const url = new URL(trimmedUrl);
      if (url.protocol !== "https:" && url.protocol !== "http:") {
        errors.base_url = "Base URL 必须以 http:// 或 https:// 开头";
      }
    } catch {
      errors.base_url = "Base URL 格式不合法";
    }
  }

  if (!values.api_key.trim()) errors.api_key = "API Key 不能为空";

  const priority = Number(values.priority);
  if (!Number.isInteger(priority)) {
    errors.priority = "优先级必须是整数";
  } else if (priority < PRIORITY_MIN || priority > PRIORITY_MAX) {
    errors.priority = `优先级范围 ${PRIORITY_MIN}–${PRIORITY_MAX}`;
  }

  return errors;
}

export function AddProviderForm({ onAdded }: AddProviderFormProps) {
  const [values, setValues] = useState(INITIAL_VALUES);
  const [errors, setErrors] = useState<FieldErrors>({});
  const [submitting, setSubmitting] = useState(false);
  const [serverError, setServerError] = useState<string | null>(null);
  const [successId, setSuccessId] = useState<string | null>(null);

  const handleSubmit = useCallback(
    async (event: React.FormEvent<HTMLFormElement>) => {
      event.preventDefault();
      setSuccessId(null);
      setServerError(null);
      const fieldErrors = validate(values);
      setErrors(fieldErrors);
      if (Object.keys(fieldErrors).length > 0) return;

      const input: ProviderInput = {
        name: values.name.trim(),
        kind: "openai",
        base_url: values.base_url.trim().replace(/\/$/, ""),
        api_key: values.api_key.trim(),
        priority: Number(values.priority),
        enabled: values.enabled,
      };
      setSubmitting(true);
      try {
        const provider = await addProvider(input);
        setSuccessId(provider.id);
        setValues(INITIAL_VALUES);
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

  return (
    <form
      noValidate
      onSubmit={handleSubmit}
      aria-label="添加 OpenAI 供应商"
      className="space-y-5 rounded-2xl border border-border bg-card p-6 shadow-apple-card"
    >
      <header className="space-y-1">
        <h3 className="text-base font-semibold leading-apple-headline tracking-apple-tight">
          添加供应商
        </h3>
        <p className="text-xs text-muted-foreground">
          OpenAI 兼容模板。优先级越小越优先；同优先级按创建时间排序。
        </p>
      </header>

      <Field
        id="provider-name"
        label="名称"
        placeholder="例如 Work OpenAI"
        value={values.name}
        onChange={(v) => setValues((s) => ({ ...s, name: v }))}
        error={errors.name}
      />

      <Field
        id="provider-base-url"
        label="Base URL"
        placeholder="https://api.openai.com"
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
        label="优先级"
        inputMode="numeric"
        placeholder="100"
        value={values.priority}
        onChange={(v) => setValues((s) => ({ ...s, priority: v }))}
        error={errors.priority}
        hint={`数字越小越优先（${PRIORITY_MIN}–${PRIORITY_MAX}）`}
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
        <span>启用此供应商（参与切换）</span>
      </label>

      <footer className="flex items-center justify-between gap-3">
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
              ? `已添加（id: ${successId.slice(0, 8)}…）`
              : ""}
        </p>
        <Button type="submit" disabled={submitting}>
          {submitting ? "添加中…" : "添加"}
        </Button>
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
