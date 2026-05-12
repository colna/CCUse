import {
  CopyOutlined,
  EyeInvisibleOutlined,
  EyeOutlined,
  ReloadOutlined,
} from "@ant-design/icons";
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
  type LocalApiEndpointConfig,
} from "@/lib/tauri";

/**
 * 仪表盘上的"本地 API"卡片：按 OpenAI-compatible / Anthropic 两个
 * 入站协议展示 Base URL + API Key，并提供轮换 / 重启动作。Key 默认
 * 遮罩，避免肩窥；重启 / 轮换由后端推送 `local_api_config_changed`
 * 事件，所以这里同时订阅事件以保持最新。
 */

type Status = "loading" | "running" | "stopped";
type Protocol = "openai" | "anthropic";
type CopyTarget = `${Protocol}-base` | `${Protocol}-key`;
type TFn = (key: string, opts?: Record<string, string | number>) => string;

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
const COPY_HINT_DURATION_MS = 1500;
const INITIAL_VISIBLE_KEYS: Record<Protocol, boolean> = {
  openai: false,
  anthropic: false,
};

const PROTOCOLS: readonly Protocol[] = ["openai", "anthropic"] as const;

export function LocalApiCard() {
  const { t } = useTranslation("providers");
  const [state, setState] = useState<CardState>(INITIAL_STATE);
  const [visibleKeys, setVisibleKeys] = useState(INITIAL_VISIBLE_KEYS);
  const [copyHint, setCopyHint] = useState<CopyTarget | null>(null);

  const refresh = useCallback(async () => {
    setState((prev) => ({ ...prev, status: "loading", error: null }));
    try {
      const config = await getLocalApiConfig();
      setState({ status: "running", config, error: null });
    } catch (err: unknown) {
      setState({ status: "stopped", config: null, error: errorMessage(err) });
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    // `.catch(() => undefined)` 消化非 Tauri 环境下 `listen()` 抛错
    // （`__TAURI_INTERNALS__` 不存在），让事件订阅失败时 UI 仍可用。
    const unlistenPromise = onLocalApiConfigChanged((config) => {
      setState({ status: "running", config, error: null });
    }).catch(() => undefined);
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
      setState({ status: "stopped", config: null, error: errorMessage(err) });
    }
  }, []);

  const handleRotate = useCallback(async () => {
    try {
      const config = await regenerateApiKey();
      setState((prev) => ({ ...prev, config, error: null }));
    } catch (err: unknown) {
      setState((prev) => ({ ...prev, error: errorMessage(err) }));
    }
  }, []);

  const handleCopy = useCallback(async (label: CopyTarget, text: string) => {
    await copyToClipboard(text);
    setCopyHint(label);
    setTimeout(() => setCopyHint(null), COPY_HINT_DURATION_MS);
  }, []);

  const { config, status, error } = state;
  const endpoints = buildProtocolConfigs(config);

  return (
    <article
      aria-labelledby="local-api-card-title"
      data-testid="local-api-card"
      className="rounded-2xl border border-[var(--app-border-secondary)] bg-[var(--app-bg-container)] p-6"
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
        <StatusBadge status={status} />
      </header>

      <div className="mt-6 space-y-6 text-sm">
        {PROTOCOLS.map((protocol, index) => {
          const endpoint = endpoints[protocol];
          const protocolLabel = t(`local_api_${protocol}_title`);
          return (
            <ProtocolConfigSection
              key={protocol}
              protocol={protocol}
              protocolLabel={protocolLabel}
              description={t(`local_api_${protocol}_desc`)}
              endpoint={endpoint}
              visible={visibleKeys[protocol]}
              separated={index > 0}
              onToggleVisible={() =>
                setVisibleKeys((current) => ({
                  ...current,
                  [protocol]: !current[protocol],
                }))
              }
              onCopyBase={() =>
                handleCopy(`${protocol}-base`, endpoint?.base_url ?? "")
              }
              onCopyKey={() =>
                handleCopy(`${protocol}-key`, endpoint?.api_key ?? "")
              }
            />
          );
        })}
      </div>

      <footer className="mt-6 flex items-center justify-between gap-3">
        <p
          aria-live="polite"
          className={cn(
            "text-xs",
            copyHint && !error
              ? "text-[var(--app-primary)]"
              : error
                ? "text-destructive"
                : "text-muted-foreground",
          )}
        >
          {error
            ? error
            : copyHint
              ? t("local_api_copied", copyHintParams(copyHint, t))
              : ""}
        </p>
        <div className="flex items-center gap-2">
          <Button
            htmlType="button"
            type="default"
            onClick={handleRotate}
            disabled={status !== "running"}
          >
            {t("local_api_rotate_key")}
          </Button>
          <Button
            htmlType="button"
            type="primary"
            onClick={handleRestart}
            disabled={status === "loading"}
            icon={<ReloadOutlined aria-label="" role="presentation" />}
          >
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
      className="inline-flex items-center gap-2 rounded-full border border-[var(--app-border-secondary)] bg-[var(--app-bg-container)] px-3 py-1 text-xs"
    >
      <span aria-hidden className={cn("size-2 rounded-full", dotClass)} />
      {text}
    </span>
  );
}

interface ProtocolConfigSectionProps {
  protocol: Protocol;
  protocolLabel: string;
  description: string;
  endpoint: LocalApiEndpointConfig | null;
  visible: boolean;
  separated: boolean;
  onToggleVisible: () => void;
  onCopyBase: () => void;
  onCopyKey: () => void;
}

function ProtocolConfigSection({
  protocol,
  protocolLabel,
  description,
  endpoint,
  visible,
  separated,
  onToggleVisible,
  onCopyBase,
  onCopyKey,
}: ProtocolConfigSectionProps) {
  const { t } = useTranslation("providers");
  const displayedKey = endpoint
    ? visible
      ? endpoint.api_key
      : maskKey(endpoint.api_key)
    : "--";
  return (
    <section
      aria-labelledby={`local-api-${protocol}-title`}
      className={cn(
        separated && "border-t border-[var(--app-border-secondary)] pt-5",
      )}
    >
      <div className="mb-4">
        <h4
          id={`local-api-${protocol}-title`}
          className="text-sm font-semibold leading-apple-headline tracking-apple-tight"
        >
          {protocolLabel}
        </h4>
        <p className="mt-1 text-xs text-muted-foreground">{description}</p>
      </div>
      <dl className="space-y-4">
        <Field
          label={t("local_api_base_url")}
          value={endpoint?.base_url ?? "--"}
          testId={`local-api-${protocol}-base-url`}
          copyable={Boolean(endpoint?.base_url)}
          onCopy={onCopyBase}
          copyAriaLabel={t("local_api_copy_protocol_aria", {
            protocol: protocolLabel,
            label: "Base URL",
          })}
        />
        <KeyField
          protocol={protocol}
          protocolLabel={protocolLabel}
          value={displayedKey}
          visible={visible}
          testId={`local-api-${protocol}-key`}
          onToggleVisible={onToggleVisible}
          copyable={Boolean(endpoint?.api_key)}
          onCopy={onCopyKey}
        />
      </dl>
    </section>
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
          className="flex-1 truncate rounded-md border border-[var(--app-border-secondary)] bg-[var(--app-bg-subtle)] px-3 py-2 font-mono text-xs"
        >
          {value}
        </code>
        <Button
          htmlType="button"
          type="text"
          shape="circle"
          aria-label={copyAriaLabel}
          onClick={onCopy}
          disabled={!copyable}
          icon={<CopyOutlined aria-label="" role="presentation" />}
        />
      </dd>
    </div>
  );
}

interface KeyFieldProps {
  protocol: Protocol;
  protocolLabel: string;
  value: string;
  visible: boolean;
  testId?: string;
  copyable: boolean;
  onToggleVisible: () => void;
  onCopy: () => void;
}

function KeyField({
  protocol,
  protocolLabel,
  value,
  visible,
  testId,
  copyable,
  onToggleVisible,
  onCopy,
}: KeyFieldProps) {
  const { t } = useTranslation("providers");
  const Icon = visible ? EyeInvisibleOutlined : EyeOutlined;
  return (
    <div>
      <dt className="text-xs uppercase tracking-[0.18em] text-muted-foreground">
        {t("local_api_key")}
      </dt>
      <dd className="mt-1 flex items-center gap-2">
        <code
          data-testid={testId}
          className="flex-1 truncate rounded-md border border-[var(--app-border-secondary)] bg-[var(--app-bg-subtle)] px-3 py-2 font-mono text-xs"
        >
          {value}
        </code>
        <Button
          htmlType="button"
          type="text"
          shape="circle"
          data-testid={`local-api-${protocol}-toggle-key`}
          aria-label={
            visible
              ? t("local_api_hide_protocol_key_aria", {
                  protocol: protocolLabel,
                })
              : t("local_api_show_protocol_key_aria", {
                  protocol: protocolLabel,
                })
          }
          onClick={onToggleVisible}
          disabled={!copyable}
          icon={<Icon aria-label="" role="presentation" />}
        />
        <Button
          htmlType="button"
          type="text"
          shape="circle"
          aria-label={t("local_api_copy_protocol_aria", {
            protocol: protocolLabel,
            label: "API Key",
          })}
          onClick={onCopy}
          disabled={!copyable}
          icon={<CopyOutlined aria-label="" role="presentation" />}
        />
      </dd>
    </div>
  );
}

function buildProtocolConfigs(
  config: LocalApiConfig | null,
): Record<Protocol, LocalApiEndpointConfig | null> {
  if (!config) return { openai: null, anthropic: null };
  return {
    openai: config.openai ?? {
      base_url: `${config.base_url.replace(/\/$/, "")}/v1`,
      api_key: config.api_key,
    },
    anthropic: config.anthropic ?? {
      base_url: config.base_url,
      api_key: config.api_key,
    },
  };
}

function copyHintParams(copyHint: CopyTarget, t: TFn) {
  const [protocol, kind] = copyHint.split("-") as [Protocol, "base" | "key"];
  return {
    protocol: t(`local_api_${protocol}_title`),
    label: kind === "base" ? "Base URL" : "API Key",
  };
}

/** 把 key 中段隐藏，只保留前缀和最后 4 字符，方便用户对照核对又不暴露 key。 */
function maskKey(key: string): string {
  if (!key) return "";
  const [head] = key.split("-");
  const suffix = key.slice(-4);
  return `${head ?? "sk"}-local---------${suffix}`;
}

function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}
