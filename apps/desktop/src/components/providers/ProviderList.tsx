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
  Check,
  FlaskConical,
  GripVertical,
  Pencil,
  RefreshCw,
  Trash2,
  X,
} from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
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

const providerKindOptions: Array<{
  kind: ProviderInput["kind"];
  label: string;
}> = [
  { kind: "openai", label: "OpenAI" },
  { kind: "anthropic", label: "Anthropic" },
  { kind: "gemini", label: "Gemini" },
  { kind: "relay", label: "Relay" },
  { kind: "custom", label: "Custom" },
];

// ─── Delete Confirmation Dialog ──────────────────────────────

interface DeleteDialogProps {
  providerName: string;
  onConfirm: () => void;
  onCancel: () => void;
}

function DeleteDialog({
  providerName,
  onConfirm,
  onCancel,
}: DeleteDialogProps) {
  const { t } = useTranslation("providers");
  const { t: tc } = useTranslation("common");
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
      <div className="mx-4 w-full max-w-sm rounded-2xl border border-border bg-card p-6 shadow-lg">
        <h3 className="text-base font-semibold">{t("delete_title")}</h3>
        <p className="mt-2 text-sm text-muted-foreground">
          {t("delete_confirm")}{" "}
          <span className="font-medium text-foreground">{providerName}</span>?{" "}
          {t("delete_undone")}
        </p>
        <div className="mt-5 flex justify-end gap-2">
          <Button variant="outline" size="sm" onClick={onCancel}>
            {tc("cancel")}
          </Button>
          <Button variant="destructive" size="sm" onClick={onConfirm}>
            {tc("delete")}
          </Button>
        </div>
      </div>
    </div>
  );
}

// ─── Inline Edit Form ────────────────────────────────────────

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
  onDelete: (id: string, name: string) => void;
  onToggleEnabled: (id: string, enabled: boolean) => void;
  onTestConnection: (id: string) => Promise<void>;
  onSaveEdit: (id: string, patch: Partial<EditState>) => Promise<void>;
}

function SortableProviderItem({
  provider,
  health,
  testing,
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
    setSaving(true);
    try {
      await onSaveEdit(provider.id, editValues);
      setEditing(false);
    } finally {
      setSaving(false);
    }
  }, [provider.id, editValues, onSaveEdit]);

  if (editing) {
    return (
      <div
        ref={setNodeRef}
        style={style}
        className="space-y-3 rounded-xl border border-primary/40 bg-card px-4 py-3 shadow-sm"
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
            onChange={(e) =>
              setEditValues((s) => ({ ...s, name: e.target.value }))
            }
            className="flex-1 rounded-md border border-border bg-background px-2 py-1 text-sm outline-none focus-visible:border-primary"
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
            onChange={(e) =>
              setEditValues((s) => ({
                ...s,
                kind: e.target.value as ProviderInput["kind"],
              }))
            }
            className="w-40 rounded-md border border-border bg-background px-2 py-1 text-sm outline-none focus-visible:border-primary"
          >
            {providerKindOptions.map((option) => (
              <option key={option.kind} value={option.kind}>
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
            onChange={(e) =>
              setEditValues((s) => ({ ...s, base_url: e.target.value }))
            }
            className="flex-1 rounded-md border border-border bg-background px-2 py-1 text-sm outline-none focus-visible:border-primary"
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
            onChange={(e) =>
              setEditValues((s) => ({ ...s, api_key: e.target.value }))
            }
            className="flex-1 rounded-md border border-border bg-background px-2 py-1 text-sm outline-none focus-visible:border-primary"
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
            onChange={(e) =>
              setEditValues((s) => ({ ...s, priority: e.target.value }))
            }
            className="w-20 rounded-md border border-border bg-background px-2 py-1 text-sm outline-none focus-visible:border-primary"
          />
          <label className="ml-4 flex items-center gap-1 text-xs">
            <input
              type="checkbox"
              checked={editValues.enabled}
              onChange={(e) =>
                setEditValues((s) => ({ ...s, enabled: e.target.checked }))
              }
              className="size-3.5 rounded border-border accent-primary"
            />
            {tc("enabled")}
          </label>
          <div className="ml-auto flex items-center gap-1">
            <Button
              variant="ghost"
              size="icon"
              className="size-7 text-primary hover:text-primary"
              onClick={handleSaveEdit}
              disabled={saving}
              aria-label={t("save_changes_aria")}
            >
              <Check className="size-3.5" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className="size-7 text-muted-foreground hover:text-foreground"
              onClick={handleCancelEdit}
              disabled={saving}
              aria-label={t("cancel_editing_aria")}
            >
              <X className="size-3.5" />
            </Button>
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
        "flex items-center gap-3 rounded-xl border border-border bg-card px-4 py-3 shadow-sm transition-shadow",
        isDragging && "z-50 shadow-lg ring-2 ring-primary/30",
      )}
    >
      <button
        {...attributes}
        {...listeners}
        className="cursor-grab touch-none text-muted-foreground hover:text-foreground"
        aria-label={t("drag_to_reorder_aria")}
      >
        <GripVertical className="size-4" />
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

      <span
        className={cn(
          "text-xs tabular-nums",
          health?.success_rate != null && health.success_rate < 0.9
            ? "text-yellow-600"
            : "text-muted-foreground",
        )}
        title={t("success_rate_title")}
      >
        {formatSuccessRate(health?.success_rate)}
      </span>

      {health?.response_time_us != null && (
        <span className="text-xs tabular-nums text-muted-foreground">
          {Math.round(health.response_time_us / 1000)}ms
        </span>
      )}

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

      <Button
        variant="ghost"
        size="icon"
        className="size-7 text-muted-foreground hover:text-foreground"
        onClick={() => onTestConnection(provider.id)}
        disabled={testing}
        aria-label={t("test_connection_provider_aria", { name: provider.name })}
      >
        {testing ? (
          <RefreshCw className="size-3.5 animate-spin" />
        ) : (
          <FlaskConical className="size-3.5" />
        )}
      </Button>

      <Button
        variant="ghost"
        size="icon"
        className="size-7 text-muted-foreground hover:text-foreground"
        onClick={handleStartEdit}
        aria-label={t("edit_provider_aria", { name: provider.name })}
      >
        <Pencil className="size-3.5" />
      </Button>

      <Button
        variant="ghost"
        size="icon"
        className="size-7 text-muted-foreground hover:text-destructive"
        onClick={() => onDelete(provider.id, provider.name)}
        aria-label={t("delete_provider_aria", { name: provider.name })}
      >
        <Trash2 className="size-3.5" />
      </Button>
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

// ─── Provider List ───────────────────────────────────────────

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
  const [error, setError] = useState<string | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<{
    id: string;
    name: string;
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
    if (!deleteTarget) return;
    try {
      await deleteProvider(deleteTarget.id);
      setProviders((prev) => prev.filter((p) => p.id !== deleteTarget.id));
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setDeleteTarget(null);
    }
  }, [deleteTarget]);

  const handleCancelDelete = useCallback(() => {
    setDeleteTarget(null);
  }, []);

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
        const latencyMs = await testProviderConnection(id);
        const provider = providers.find((item) => item.id === id);
        setHealthMap((current) => ({
          ...current,
          [id]: {
            provider_id: id,
            provider_name: provider?.name ?? id,
            status: "healthy",
            success_rate: current[id]?.success_rate ?? 1,
            response_time_us: latencyMs * 1000,
          },
        }));
        setError(null);
      } catch (err: unknown) {
        setError(err instanceof Error ? err.message : String(err));
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
      <div className="rounded-xl border border-destructive/30 bg-card p-4 text-sm text-destructive">
        {error}
      </div>
    );
  }

  if (providers.length === 0) {
    return (
      <div className="rounded-xl border border-dashed border-border bg-card/50 px-6 py-8 text-center text-sm text-muted-foreground">
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
          <div className="space-y-2">
            {providers.map((provider) => (
              <SortableProviderItem
                key={provider.id}
                provider={provider}
                health={healthMap[provider.id]}
                testing={testingIds[provider.id] ?? false}
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
          onConfirm={handleConfirmDelete}
          onCancel={handleCancelDelete}
        />
      )}
    </>
  );
}
