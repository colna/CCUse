import { Copy, Eye, EyeOff, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import {
  copyToClipboard,
  getLocalApiConfig,
  onLocalApiConfigChanged,
  regenerateApiKey,
  restartProxy,
  type LocalApiConfig,
} from "@/lib/tauri";

type Status = "loading" | "running" | "stopped";

interface CardState {
  status: Status;
  config: LocalApiConfig | null;
  error: string | null;
}

const INITIAL_STATE: CardState = {
  status: "loading",
  config: null,
  error: null,
};

function maskKey(key: string): string {
  if (!key) return "";
  const [head] = key.split("-");
  const suffix = key.slice(-4);
  return `${head ?? "sk"}-local---------${suffix}`;
}

export function LocalApiCard() {
  const { t } = useTranslation("providers");
  const [state, setState] = useState<CardState>(INITIAL_STATE);
  const [keyVisible, setKeyVisible] = useState(false);
  const [copyHint, setCopyHint] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setState((prev) => ({ ...prev, status: "loading", error: null }));
    try {
      const config = await getLocalApiConfig();
      setState({ status: "running", config, error: null });
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : String(err);
      setState({ status: "stopped", config: null, error: message });
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    const unlistenPromise = onLocalApiConfigChanged((config) => {
      setState({ status: "running", config, error: null });
    }).catch(() => null);
    return () => {
      void unlistenPromise.then((unlisten) => unlisten?.());
    };
  }, []);

  const handleRestart = useCallback(async () => {
    setState((prev) => ({ ...prev, status: "loading", error: null }));
    try {
      const config = await restartProxy();
      setState({ status: "running", config, error: null });
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : String(err);
      setState({ status: "stopped", config: null, error: message });
    }
  }, []);

  const handleRotate = useCallback(async () => {
    try {
      const config = await regenerateApiKey();
      setState((prev) => ({ ...prev, config, error: null }));
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : String(err);
      setState((prev) => ({ ...prev, error: message }));
    }
  }, []);

  const handleCopy = useCallback(async (label: string, text: string) => {
    await copyToClipboard(text);
    setCopyHint(label);
    setTimeout(() => setCopyHint(null), 1500);
  }, []);

  const config = state.config;
  const displayedKey = config
    ? keyVisible
      ? config.api_key
      : maskKey(config.api_key)
    : "--";

  return (
    <article
      aria-labelledby="local-api-card-title"
      data-testid="local-api-card"
      className="rounded-2xl border border-border bg-card p-6 shadow-apple-card"
    >
      <header className="flex items-start justify-between gap-4">
        <div>
          <h3
            id="local-api-card-title"
            className="text-base font-semibold leading-apple-headline tracking-apple-tight"
          >
            {t("local_api_title")}
          </h3>
          <p className="mt-1 text-xs text-muted-foreground">
            {t("local_api_desc")}
          </p>
        </div>
        <StatusBadge status={state.status} />
      </header>

      <dl className="mt-6 space-y-5 text-sm">
        <Field
          label={t("local_api_base_url")}
          value={config?.base_url ?? "--"}
          testId="local-api-base-url"
          copyable={Boolean(config?.base_url)}
          onCopy={() => handleCopy("base", config?.base_url ?? "")}
          copyAriaLabel={t("local_api_copy_aria", { label: "Base URL" })}
        />
        <KeyField
          value={displayedKey}
          visible={keyVisible}
          testId="local-api-key"
          onToggleVisible={() => setKeyVisible((v) => !v)}
          copyable={Boolean(config?.api_key)}
          onCopy={() => handleCopy("key", config?.api_key ?? "")}
        />
      </dl>

      <footer className="mt-6 flex items-center justify-between gap-3">
        <p
          aria-live="polite"
          className={cn(
            "text-xs",
            copyHint && !state.error
              ? "text-primary"
              : state.error
                ? "text-destructive"
                : "text-muted-foreground",
          )}
        >
          {state.error
            ? state.error
            : copyHint === "base"
              ? t("local_api_base_copied")
              : copyHint === "key"
                ? t("local_api_key_copied")
                : ""}
        </p>
        <div className="flex items-center gap-2">
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={handleRotate}
            disabled={state.status !== "running"}
          >
            {t("local_api_rotate_key")}
          </Button>
          <Button
            type="button"
            size="sm"
            onClick={handleRestart}
            disabled={state.status === "loading"}
          >
            <RefreshCw className="mr-2 size-4" aria-hidden />
            {t("local_api_restart")}
          </Button>
        </div>
      </footer>
    </article>
  );
}

function StatusBadge({ status }: { status: Status }) {
  const { t } = useTranslation("providers");
  const text =
    status === "running"
      ? t("local_api_status_running")
      : status === "loading"
        ? t("local_api_status_loading")
        : t("local_api_status_stopped");
  const dotClass =
    status === "running"
      ? "bg-emerald-500"
      : status === "loading"
        ? "bg-amber-500"
        : "bg-muted-foreground/40";
  return (
    <span
      role="status"
      className="inline-flex items-center gap-2 rounded-full border border-border bg-background px-3 py-1 text-xs"
    >
      <span aria-hidden className={cn("size-2 rounded-full", dotClass)} />
      {text}
    </span>
  );
}

interface FieldProps {
  label: string;
  value: string;
  testId?: string;
  copyable: boolean;
  onCopy: () => void;
  copyAriaLabel: string;
}

function Field({
  label,
  value,
  testId,
  copyable,
  onCopy,
  copyAriaLabel,
}: FieldProps) {
  return (
    <div>
      <dt className="text-xs uppercase tracking-[0.18em] text-muted-foreground">
        {label}
      </dt>
      <dd className="mt-1 flex items-center gap-2">
        <code
          data-testid={testId}
          className="flex-1 truncate rounded-md border border-border bg-muted/40 px-3 py-2 font-mono text-xs"
        >
          {value}
        </code>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          aria-label={copyAriaLabel}
          onClick={onCopy}
          disabled={!copyable}
        >
          <Copy className="size-4" aria-hidden />
        </Button>
      </dd>
    </div>
  );
}

interface KeyFieldProps {
  value: string;
  visible: boolean;
  testId?: string;
  copyable: boolean;
  onToggleVisible: () => void;
  onCopy: () => void;
}

function KeyField({
  value,
  visible,
  testId,
  copyable,
  onToggleVisible,
  onCopy,
}: KeyFieldProps) {
  const { t } = useTranslation("providers");
  const Icon = visible ? EyeOff : Eye;
  return (
    <div>
      <dt className="text-xs uppercase tracking-[0.18em] text-muted-foreground">
        {t("local_api_key")}
      </dt>
      <dd className="mt-1 flex items-center gap-2">
        <code
          data-testid={testId}
          className="flex-1 truncate rounded-md border border-border bg-muted/40 px-3 py-2 font-mono text-xs"
        >
          {value}
        </code>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          data-testid="local-api-toggle-key"
          aria-label={
            visible
              ? t("local_api_hide_key_aria")
              : t("local_api_show_key_aria")
          }
          onClick={onToggleVisible}
          disabled={!copyable}
        >
          <Icon className="size-4" aria-hidden />
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          aria-label={t("local_api_copy_key_aria")}
          onClick={onCopy}
          disabled={!copyable}
        >
          <Copy className="size-4" aria-hidden />
        </Button>
      </dd>
    </div>
  );
}
