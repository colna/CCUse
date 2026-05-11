import { useCallback, useState } from "react";
import {
  CheckOutlined,
  CloseOutlined,
  DeleteOutlined,
  EditOutlined,
  ExperimentOutlined,
  HolderOutlined,
  LoadingOutlined,
  ReloadOutlined,
} from "@ant-design/icons";
import { Tooltip } from "antd";
import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { PROVIDER_KIND_OPTIONS } from "@/lib/providerKinds";
import { cn } from "@/lib/utils";
import type { HealthSnapshot, Provider, ProviderInput } from "@/lib/tauri";

import {
  formatSuccessRate,
  healthStatusLabel,
  statusDotColor,
} from "./healthDisplay";

/**
 * 列表里"一行供应商"的两种形态：只读卡片 + 内联编辑。
 * 拆出来是因为这两种 UI 的字段几乎完全独立，但又共享 dnd-kit 的拖拽
 * 句柄、保存中的禁用态等；保留在同一组件里能避免 sortable id 错位。
 */

export interface EditState {
  name: string;
  kind: ProviderInput["kind"];
  base_url: string;
  api_key: string;
  priority: string;
  enabled: boolean;
}

interface SortableProviderItemProps {
  provider: Provider;
  health?: HealthSnapshot;
  testing?: boolean;
  deleting?: boolean;
  onDelete: (id: string, name: string) => void;
  onToggleEnabled: (id: string, enabled: boolean) => void;
  onTestConnection: (id: string) => Promise<void>;
  onSaveEdit: (id: string, patch: EditState) => Promise<void>;
}

/** 列表里的输入框统一样式（编辑态用）。 */
const INPUT_CLASS =
  "flex-1 rounded-md border border-[var(--app-border)] bg-[var(--app-bg-container)] px-2 py-1 text-sm outline-none focus-visible:border-[var(--app-primary)] disabled:cursor-not-allowed disabled:opacity-60";

export function SortableProviderItem({
  provider,
  health,
  testing,
  deleting,
  onDelete,
  onToggleEnabled,
  onTestConnection,
  onSaveEdit,
}: SortableProviderItemProps) {
  const { t } = useTranslation("providers");
  const { t: tc } = useTranslation("common");
  const [editing, setEditing] = useState(false);
  const [editValues, setEditValues] = useState<EditState>(() =>
    initialEditState(provider),
  );
  const [saving, setSaving] = useState(false);

  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: provider.id });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
  };

  const handleStartEdit = useCallback(() => {
    // 进入编辑时永远从最新 provider 重新初始化，避免上次取消后 stale。
    setEditValues(initialEditState(provider));
    setEditing(true);
  }, [provider]);

  const handleSaveEdit = useCallback(async () => {
    if (saving) return;
    setSaving(true);
    try {
      await onSaveEdit(provider.id, editValues);
      setEditing(false);
    } finally {
      setSaving(false);
    }
  }, [provider.id, editValues, saving, onSaveEdit]);

  if (editing) {
    return (
      <div
        ref={setNodeRef}
        style={style}
        aria-busy={saving}
        className="border-[var(--app-primary)]/40 space-y-3 rounded-2xl border bg-[var(--app-bg-container)] px-5 py-4"
      >
        <LabeledField
          id={`edit-name-${provider.id}`}
          label={t("edit_name_label")}
        >
          <input
            id={`edit-name-${provider.id}`}
            type="text"
            value={editValues.name}
            disabled={saving}
            onChange={(e) =>
              setEditValues((s) => ({ ...s, name: e.target.value }))
            }
            className={INPUT_CLASS}
          />
        </LabeledField>

        <LabeledField
          id={`edit-kind-${provider.id}`}
          label={t("edit_kind_label")}
        >
          <select
            id={`edit-kind-${provider.id}`}
            value={editValues.kind}
            disabled={saving}
            onChange={(e) =>
              setEditValues((s) => ({
                ...s,
                kind: e.target.value as ProviderInput["kind"],
              }))
            }
            className={cn(INPUT_CLASS, "w-40 flex-none")}
          >
            {PROVIDER_KIND_OPTIONS.map((option) => (
              <option
                key={option.kind}
                value={option.kind}
                disabled={!option.supported}
              >
                {option.label}
              </option>
            ))}
          </select>
        </LabeledField>

        <LabeledField
          id={`edit-url-${provider.id}`}
          label={t("edit_url_label")}
        >
          <input
            id={`edit-url-${provider.id}`}
            type="text"
            value={editValues.base_url}
            disabled={saving}
            onChange={(e) =>
              setEditValues((s) => ({ ...s, base_url: e.target.value }))
            }
            className={INPUT_CLASS}
          />
        </LabeledField>

        <LabeledField
          id={`edit-api-key-${provider.id}`}
          label={t("field_api_key")}
        >
          <input
            id={`edit-api-key-${provider.id}`}
            type="password"
            value={editValues.api_key}
            placeholder={t("edit_api_key_placeholder")}
            disabled={saving}
            onChange={(e) =>
              setEditValues((s) => ({ ...s, api_key: e.target.value }))
            }
            className={INPUT_CLASS}
          />
        </LabeledField>

        <div className="flex items-center gap-2">
          <label
            htmlFor={`edit-priority-${provider.id}`}
            className="w-16 shrink-0 text-xs text-muted-foreground"
          >
            {t("edit_priority_label")}
          </label>
          <input
            id={`edit-priority-${provider.id}`}
            type="text"
            inputMode="numeric"
            value={editValues.priority}
            disabled={saving}
            onChange={(e) =>
              setEditValues((s) => ({ ...s, priority: e.target.value }))
            }
            className={cn(INPUT_CLASS, "w-20 flex-none")}
          />
          <label className="ml-4 flex items-center gap-1 text-xs">
            <input
              type="checkbox"
              checked={editValues.enabled}
              disabled={saving}
              onChange={(e) =>
                setEditValues((s) => ({ ...s, enabled: e.target.checked }))
              }
              className="size-3.5 rounded border-border accent-primary"
            />
            {tc("enabled")}
          </label>
          <div className="ml-auto flex items-center gap-1.5">
            <Button
              type="text"
              size="small"
              shape="circle"
              onClick={handleSaveEdit}
              disabled={saving}
              aria-label={t("save_changes_aria")}
              icon={
                saving ? (
                  <LoadingOutlined
                    className="animate-spin"
                    aria-label=""
                    role="presentation"
                  />
                ) : (
                  <CheckOutlined aria-label="" role="presentation" />
                )
              }
            />
            <Button
              type="text"
              size="small"
              shape="circle"
              onClick={() => setEditing(false)}
              disabled={saving}
              aria-label={t("cancel_editing_aria")}
              icon={<CloseOutlined aria-label="" role="presentation" />}
            />
          </div>
        </div>
      </div>
    );
  }

  return (
    <div
      ref={setNodeRef}
      style={style}
      className={cn(
        "flex items-center gap-4 rounded-2xl border border-[var(--app-border-secondary)] bg-[var(--app-bg-container)] px-5 py-3.5 transition-shadow",
        isDragging && "ring-[var(--app-primary)]/30 z-50 shadow-lg ring-2",
      )}
    >
      <button
        {...attributes}
        {...listeners}
        className="cursor-grab touch-none text-muted-foreground hover:text-foreground"
        aria-label={t("drag_to_reorder_aria")}
      >
        <HolderOutlined className="text-base" />
      </button>

      <span
        className={cn(
          "size-2.5 shrink-0 rounded-full",
          statusDotColor(health?.status),
        )}
        title={health?.status ?? "unknown"}
      />

      <div className="min-w-0 flex-1">
        <p className="truncate text-sm font-medium">{provider.name}</p>
        <p className="truncate text-xs text-muted-foreground">
          {provider.kind} ·{" "}
          {t("priority_display", { value: provider.priority })}
        </p>
      </div>

      <div className="flex items-center gap-3 text-xs tabular-nums">
        <span
          className={cn(
            health?.success_rate != null && health.success_rate < 0.9
              ? "text-yellow-600"
              : "text-muted-foreground",
          )}
          title={t("success_rate_title")}
        >
          {formatSuccessRate(health?.success_rate)}
        </span>

        {health?.response_time_us != null && (
          <span className="text-muted-foreground">
            {Math.round(health.response_time_us / 1000)}ms
          </span>
        )}

        {health?.status && (
          <span className="text-muted-foreground">
            {healthStatusLabel(health.status)}
          </span>
        )}
      </div>

      <span
        className="mx-1 h-6 w-px shrink-0 bg-[var(--app-border-secondary)]"
        aria-hidden
      />

      <label className="flex items-center gap-1 text-xs">
        <input
          type="checkbox"
          checked={provider.enabled}
          onChange={(e) => onToggleEnabled(provider.id, e.target.checked)}
          className="size-3.5 rounded border-border accent-primary"
          aria-label={
            provider.enabled
              ? t("disable_provider_aria", { name: provider.name })
              : t("enable_provider_aria", { name: provider.name })
          }
        />
      </label>

      <div className="flex items-center gap-1">
        <Tooltip
          title={t("test_connection_provider_aria", { name: provider.name })}
        >
          <Button
            type="text"
            size="small"
            shape="circle"
            onClick={() => onTestConnection(provider.id)}
            disabled={testing}
            aria-label={t("test_connection_provider_aria", {
              name: provider.name,
            })}
            icon={
              testing ? (
                <ReloadOutlined spin aria-label="" role="presentation" />
              ) : (
                <ExperimentOutlined aria-label="" role="presentation" />
              )
            }
          />
        </Tooltip>
        <Tooltip title={t("edit_provider_aria", { name: provider.name })}>
          <Button
            type="text"
            size="small"
            shape="circle"
            onClick={handleStartEdit}
            aria-label={t("edit_provider_aria", { name: provider.name })}
            icon={<EditOutlined aria-label="" role="presentation" />}
          />
        </Tooltip>
        <Tooltip title={t("delete_provider_aria", { name: provider.name })}>
          <Button
            type="text"
            size="small"
            shape="circle"
            danger
            onClick={() => onDelete(provider.id, provider.name)}
            disabled={deleting}
            aria-label={t("delete_provider_aria", { name: provider.name })}
            icon={
              deleting ? (
                <LoadingOutlined
                  className="animate-spin"
                  aria-label=""
                  role="presentation"
                />
              ) : (
                <DeleteOutlined aria-label="" role="presentation" />
              )
            }
          />
        </Tooltip>
      </div>
    </div>
  );
}

function initialEditState(provider: Provider): EditState {
  return {
    name: provider.name,
    kind: provider.kind,
    base_url: provider.base_url,
    // 编辑时永远以空 key 起步；空串表示"沿用旧 key"，避免明文回填。
    api_key: "",
    priority: String(provider.priority),
    enabled: provider.enabled,
  };
}

function LabeledField({
  id,
  label,
  children,
}: {
  id: string;
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-center gap-2">
      <label
        htmlFor={id}
        className="w-16 shrink-0 text-xs text-muted-foreground"
      >
        {label}
      </label>
      {children}
    </div>
  );
}
