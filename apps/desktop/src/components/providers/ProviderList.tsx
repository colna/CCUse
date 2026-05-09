import { useCallback, useEffect, useState } from "react";
import {
  DndContext,
  closestCenter,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import {
  arrayMove,
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
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
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { PROVIDER_KIND_OPTIONS } from "@/lib/providerKinds";
import { cn } from "@/lib/utils";
import {
  listProviders,
  deleteProvider,
  updateProvider,
  getHealthSnapshot,
  onProviderStatusChanged,
  testProviderConnection,
  type Provider,
  type ProviderInput,
  type HealthSnapshot,
  type StreamCheckResult,
} from "@/lib/tauri";

function statusColor(status?: string): string {
  switch (status) {
    case "healthy":
      return "bg-green-500";
    case "degraded":
      return "bg-yellow-500";
    case "down":
      return "bg-red-500";
    default:
      return "bg-muted-foreground/40";
  }
}

function formatSuccessRate(rate?: number): string {
  if (rate == null) return "--";
  return `${(rate * 100).toFixed(1)}%`;
}

function streamStatusLabel(status?: StreamCheckResult["status"]): string {
  switch (status) {
    case "operational":
      return "正常";
    case "degraded":
      return "降级";
    case "failed":
      return "失败";
    default:
      return "--";
  }
}

interface DeleteDialogProps {
  providerName: string;
  deleting: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

function DeleteDialog({
  providerName,
  deleting,
  onConfirm,
  onCancel,
}: DeleteDialogProps) {
  const { t } = useTranslation("providers");
  const { t: tc } = useTranslation("common");
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
      <div
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="delete-provider-dialog-title"
        aria-busy={deleting}
        className="mx-4 w-full max-w-sm rounded-2xl border border-[var(--ant-color-border-secondary,rgba(0,0,0,0.06))] bg-[var(--ant-color-bg-elevated,#fff)] p-6 shadow-xl"
      >
        <h3
          id="delete-provider-dialog-title"
          className="text-base font-semibold"
        >
          {t("delete_title")}
        </h3>
        <p className="mt-2 text-sm text-muted-foreground">
          {t("delete_confirm")}{" "}
          <span className="font-medium text-foreground">{providerName}</span>?{" "}
          {t("delete_undone")}
        </p>
        <div className="mt-5 flex justify-end gap-2">
          <Button type="default" onClick={onCancel} disabled={deleting}>
            {tc("cancel")}
          </Button>
          <Button
            type="primary"
            danger
            onClick={onConfirm}
            disabled={deleting}
            icon={
              deleting ? (
                <LoadingOutlined
                  className="animate-spin"
                  aria-label=""
                  role="presentation"
                />
              ) : undefined
            }
          >
            {deleting ? t("deleting") : tc("delete")}
          </Button>
        </div>
      </div>
    </div>
  );
}

interface ProviderErrorDialogProps {
  title: string;
  providerName: string;
  message: string;
  onClose: () => void;
}

function ProviderErrorDialog({
  title,
  providerName,
  message,
  onClose,
}: ProviderErrorDialogProps) {
  const { t: tc } = useTranslation("common");
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
      <div
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="provider-error-dialog-title"
        aria-describedby="provider-error-dialog-message"
        className="mx-4 w-full max-w-sm rounded-2xl border border-[var(--ant-color-error-border,rgba(255,77,79,0.4))] bg-[var(--ant-color-bg-elevated,#fff)] p-6 shadow-xl"
      >
        <h3
          id="provider-error-dialog-title"
          className="text-base font-semibold text-foreground"
        >
          {title}
        </h3>
        <p className="mt-2 text-sm text-muted-foreground">{providerName}</p>
        <pre
          id="provider-error-dialog-message"
          className="mt-3 max-h-44 overflow-auto whitespace-pre-wrap rounded-md bg-[var(--ant-color-fill-quaternary,rgba(0,0,0,0.02))] p-3 text-xs text-destructive"
        >
          {message}
        </pre>
        <div className="mt-5 flex justify-end">
          <Button type="default" onClick={onClose}>
            {tc("close")}
          </Button>
        </div>
      </div>
    </div>
  );
}

interface EditState {
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
  onSaveEdit: (id: string, patch: Partial<EditState>) => Promise<void>;
}

function SortableProviderItem({
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
  const [editValues, setEditValues] = useState<EditState>({
    name: provider.name,
    kind: provider.kind,
    base_url: provider.base_url,
    api_key: "",
    priority: String(provider.priority),
    enabled: provider.enabled,
  });
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
    setEditValues({
      name: provider.name,
      kind: provider.kind,
      base_url: provider.base_url,
      api_key: "",
      priority: String(provider.priority),
      enabled: provider.enabled,
    });
    setEditing(true);
  }, [provider]);

  const handleCancelEdit = useCallback(() => {
    setEditing(false);
  }, []);

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

  const inputClass =
    "flex-1 rounded-md border border-[var(--ant-color-border,rgba(0,0,0,0.08))] bg-[var(--ant-color-bg-container,#fff)] px-2 py-1 text-sm outline-none focus-visible:border-[var(--ant-color-primary,#0071e3)] disabled:cursor-not-allowed disabled:opacity-60";

  if (editing) {
    return (
      <div
        ref={setNodeRef}
        style={style}
        aria-busy={saving}
        className="border-[var(--ant-color-primary,#0071e3)]/40 space-y-3 rounded-2xl border bg-[var(--ant-color-bg-container,#fff)] px-5 py-4"
      >
        <div className="flex items-center gap-2">
          <label
            htmlFor={`edit-name-${provider.id}`}
            className="w-16 shrink-0 text-xs text-muted-foreground"
          >
            {t("edit_name_label")}
          </label>
          <input
            id={`edit-name-${provider.id}`}
            type="text"
            value={editValues.name}
            disabled={saving}
            onChange={(e) =>
              setEditValues((s) => ({ ...s, name: e.target.value }))
            }
            className={inputClass}
          />
        </div>
        <div className="flex items-center gap-2">
          <label
            htmlFor={`edit-kind-${provider.id}`}
            className="w-16 shrink-0 text-xs text-muted-foreground"
          >
            {t("edit_kind_label")}
          </label>
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
            className={cn(inputClass, "w-40 flex-none")}
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
        </div>
        <div className="flex items-center gap-2">
          <label
            htmlFor={`edit-url-${provider.id}`}
            className="w-16 shrink-0 text-xs text-muted-foreground"
          >
            {t("edit_url_label")}
          </label>
          <input
            id={`edit-url-${provider.id}`}
            type="text"
            value={editValues.base_url}
            disabled={saving}
            onChange={(e) =>
              setEditValues((s) => ({ ...s, base_url: e.target.value }))
            }
            className={inputClass}
          />
        </div>
        <div className="flex items-center gap-2">
          <label
            htmlFor={`edit-api-key-${provider.id}`}
            className="w-16 shrink-0 text-xs text-muted-foreground"
          >
            {t("field_api_key")}
          </label>
          <input
            id={`edit-api-key-${provider.id}`}
            type="password"
            value={editValues.api_key}
            placeholder={t("edit_api_key_placeholder")}
            disabled={saving}
            onChange={(e) =>
              setEditValues((s) => ({ ...s, api_key: e.target.value }))
            }
            className={inputClass}
          />
        </div>
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
            className={cn(inputClass, "w-20 flex-none")}
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
              onClick={handleCancelEdit}
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
        "flex items-center gap-4 rounded-2xl border border-[var(--ant-color-border-secondary,rgba(0,0,0,0.06))] bg-[var(--ant-color-bg-container,#fff)] px-5 py-3.5 transition-shadow",
        isDragging &&
          "ring-[var(--ant-color-primary,#0071e3)]/30 z-50 shadow-lg ring-2",
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
          statusColor(health?.status),
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
            {streamStatusLabel(
              health.status === "healthy"
                ? "operational"
                : health.status === "degraded"
                  ? "degraded"
                  : "failed",
            )}
          </span>
        )}
      </div>

      <span
        className="mx-1 h-6 w-px shrink-0 bg-[var(--ant-color-border-secondary,rgba(0,0,0,0.06))]"
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

function providerToInput(
  provider: Provider,
  overrides: Partial<ProviderInput> = {},
): ProviderInput {
  return {
    name: provider.name,
    kind: provider.kind,
    base_url: provider.base_url,
    api_key: "",
    priority: provider.priority,
    enabled: provider.enabled,
    monthly_quota: provider.monthly_quota ?? null,
    rate_limit_rpm: provider.rate_limit_rpm ?? null,
    cost_per_1k_tokens: provider.cost_per_1k_tokens ?? null,
    ...overrides,
  };
}

interface ProviderListProps {
  refreshKey?: number;
}

export function ProviderList({ refreshKey }: ProviderListProps) {
  const { t } = useTranslation("providers");
  const [providers, setProviders] = useState<Provider[]>([]);
  const [healthMap, setHealthMap] = useState<Record<string, HealthSnapshot>>(
    {},
  );
  const [testingIds, setTestingIds] = useState<Record<string, boolean>>({});
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<{
    id: string;
    name: string;
  } | null>(null);
  const [testErrorDialog, setTestErrorDialog] = useState<{
    providerName: string;
    message: string;
  } | null>(null);

  const fetchProviders = useCallback(async () => {
    try {
      const list = await listProviders();
      setProviders(list);
      setError(null);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, []);

  const fetchHealth = useCallback(async () => {
    try {
      const snap = await getHealthSnapshot();
      const map: Record<string, HealthSnapshot> = {};
      for (const s of snap.providers) {
        map[s.provider_id] = s;
      }
      setHealthMap(map);
    } catch {
      // Health snapshot not critical
    }
  }, []);

  useEffect(() => {
    fetchProviders();
    fetchHealth();
  }, [fetchProviders, fetchHealth, refreshKey]);

  useEffect(() => {
    const id = setInterval(fetchHealth, 5000);
    return () => clearInterval(id);
  }, [fetchHealth]);

  useEffect(() => {
    const unlistenPromise = onProviderStatusChanged((event) => {
      setHealthMap((current) => ({
        ...current,
        [event.provider_id]: {
          provider_id: event.provider_id,
          provider_name: event.provider_name,
          status: event.new_status,
          success_rate: event.success_rate,
          response_time_us:
            current[event.provider_id]?.response_time_us ?? null,
        },
      }));
      void fetchHealth();
    }).catch(() => null);
    return () => {
      void unlistenPromise.then((unlisten) => unlisten?.());
    };
  }, [fetchHealth]);

  const sensors = useSensors(
    useSensor(PointerSensor),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    }),
  );

  const handleDragEnd = useCallback(
    async (event: DragEndEvent) => {
      const { active, over } = event;
      if (!over || active.id === over.id) return;

      const oldIndex = providers.findIndex((p) => p.id === active.id);
      const newIndex = providers.findIndex((p) => p.id === over.id);
      const reordered = arrayMove(providers, oldIndex, newIndex);

      setProviders(reordered);

      for (let i = 0; i < reordered.length; i++) {
        const p = reordered[i];
        const newPriority = (i + 1) * 10;
        if (p.priority !== newPriority) {
          try {
            await updateProvider(p.id, {
              ...providerToInput(p),
              priority: newPriority,
            });
          } catch {
            fetchProviders();
            return;
          }
        }
      }
      fetchProviders();
    },
    [providers, fetchProviders],
  );

  const handleRequestDelete = useCallback((id: string, name: string) => {
    setDeleteTarget({ id, name });
  }, []);

  const handleConfirmDelete = useCallback(async () => {
    if (!deleteTarget || deletingId) return;
    setDeletingId(deleteTarget.id);
    try {
      await deleteProvider(deleteTarget.id);
      setProviders((prev) => prev.filter((p) => p.id !== deleteTarget.id));
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setDeletingId(null);
      setDeleteTarget(null);
    }
  }, [deleteTarget, deletingId]);

  const handleCancelDelete = useCallback(() => {
    if (deletingId) return;
    setDeleteTarget(null);
  }, [deletingId]);

  const handleToggleEnabled = useCallback(
    async (id: string, enabled: boolean) => {
      const provider = providers.find((p) => p.id === id);
      if (!provider) return;
      const input = providerToInput(provider, { enabled });
      try {
        const updated = await updateProvider(id, input);
        setProviders((prev) => prev.map((p) => (p.id === id ? updated : p)));
      } catch (err: unknown) {
        setError(err instanceof Error ? err.message : String(err));
      }
    },
    [providers],
  );

  const handleTestConnection = useCallback(
    async (id: string) => {
      setTestingIds((current) => ({ ...current, [id]: true }));
      try {
        const result = await testProviderConnection(id);
        const provider = providers.find((item) => item.id === id);
        setHealthMap((current) => ({
          ...current,
          [id]: {
            provider_id: id,
            provider_name: provider?.name ?? id,
            status: result.success
              ? result.status === "degraded"
                ? "degraded"
                : "healthy"
              : "down",
            success_rate: result.success ? 1 : 0,
            response_time_us:
              result.response_time_ms != null
                ? result.response_time_ms * 1000
                : null,
          },
        }));
        if (!result.success) {
          setTestErrorDialog({
            providerName: provider?.name ?? id,
            message: [
              result.message,
              result.http_status != null ? `HTTP ${result.http_status}` : null,
              result.error_category
                ? `Category: ${result.error_category}`
                : null,
              result.model_used ? `Model: ${result.model_used}` : null,
            ]
              .filter(Boolean)
              .join("\n"),
          });
        } else {
          setError(null);
        }
      } catch (err: unknown) {
        const provider = providers.find((item) => item.id === id);
        setTestErrorDialog({
          providerName: provider?.name ?? id,
          message: err instanceof Error ? err.message : String(err),
        });
      } finally {
        setTestingIds((current) => ({ ...current, [id]: false }));
      }
    },
    [providers],
  );

  const handleSaveEdit = useCallback(
    async (id: string, patch: Partial<EditState>) => {
      const provider = providers.find((p) => p.id === id);
      if (!provider) return;
      const input = providerToInput(provider, {
        name: (patch.name ?? provider.name).trim(),
        kind: patch.kind ?? provider.kind,
        base_url: (patch.base_url ?? provider.base_url)
          .trim()
          .replace(/\/$/, ""),
        api_key: (patch.api_key ?? "").trim(),
        priority: patch.priority ? Number(patch.priority) : provider.priority,
        enabled: patch.enabled ?? provider.enabled,
      });
      try {
        const updated = await updateProvider(id, input);
        setProviders((prev) => prev.map((p) => (p.id === id ? updated : p)));
      } catch (err: unknown) {
        setError(err instanceof Error ? err.message : String(err));
      }
    },
    [providers],
  );

  if (error) {
    return (
      <div className="rounded-2xl border border-[var(--ant-color-error-border,rgba(255,77,79,0.4))] bg-[var(--ant-color-bg-container,#fff)] p-4 text-sm text-destructive">
        {error}
      </div>
    );
  }

  if (providers.length === 0) {
    return (
      <div className="rounded-2xl border border-dashed border-[var(--ant-color-border-secondary,rgba(0,0,0,0.06))] bg-[var(--ant-color-fill-quaternary,rgba(0,0,0,0.02))] px-6 py-10 text-center text-sm text-muted-foreground">
        {t("no_providers")}
      </div>
    );
  }

  return (
    <>
      <DndContext
        sensors={sensors}
        collisionDetection={closestCenter}
        onDragEnd={handleDragEnd}
      >
        <SortableContext
          items={providers.map((p) => p.id)}
          strategy={verticalListSortingStrategy}
        >
          <div className="space-y-2.5">
            {providers.map((provider) => (
              <SortableProviderItem
                key={provider.id}
                provider={provider}
                health={healthMap[provider.id]}
                testing={testingIds[provider.id] ?? false}
                deleting={deletingId === provider.id}
                onDelete={handleRequestDelete}
                onToggleEnabled={handleToggleEnabled}
                onTestConnection={handleTestConnection}
                onSaveEdit={handleSaveEdit}
              />
            ))}
          </div>
        </SortableContext>
      </DndContext>

      {deleteTarget && (
        <DeleteDialog
          providerName={deleteTarget.name}
          deleting={deletingId === deleteTarget.id}
          onConfirm={handleConfirmDelete}
          onCancel={handleCancelDelete}
        />
      )}

      {testErrorDialog && (
        <ProviderErrorDialog
          title={t("test_connection_failed_title")}
          providerName={testErrorDialog.providerName}
          message={testErrorDialog.message}
          onClose={() => setTestErrorDialog(null)}
        />
      )}
    </>
  );
}
