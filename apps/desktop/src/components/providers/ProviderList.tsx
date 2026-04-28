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
import { GripVertical, Pencil, Trash2, Check, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import {
  listProviders,
  deleteProvider,
  updateProvider,
  getHealthSnapshot,
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
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
      <div className="mx-4 w-full max-w-sm rounded-2xl border border-border bg-card p-6 shadow-lg">
        <h3 className="text-base font-semibold">Delete Provider</h3>
        <p className="mt-2 text-sm text-muted-foreground">
          Are you sure you want to delete{" "}
          <span className="font-medium text-foreground">{providerName}</span>?
          This action cannot be undone.
        </p>
        <div className="mt-5 flex justify-end gap-2">
          <Button variant="outline" size="sm" onClick={onCancel}>
            Cancel
          </Button>
          <Button variant="destructive" size="sm" onClick={onConfirm}>
            Delete
          </Button>
        </div>
      </div>
    </div>
  );
}

// ─── Inline Edit Form ────────────────────────────────────────

interface EditState {
  name: string;
  base_url: string;
  priority: string;
  enabled: boolean;
}

interface SortableProviderItemProps {
  provider: Provider;
  health?: HealthSnapshot;
  onDelete: (id: string, name: string) => void;
  onToggleEnabled: (id: string, enabled: boolean) => void;
  onSaveEdit: (id: string, patch: Partial<EditState>) => Promise<void>;
}

function SortableProviderItem({
  provider,
  health,
  onDelete,
  onToggleEnabled,
  onSaveEdit,
}: SortableProviderItemProps) {
  const [editing, setEditing] = useState(false);
  const [editValues, setEditValues] = useState<EditState>({
    name: provider.name,
    base_url: provider.base_url,
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
      base_url: provider.base_url,
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
            Name
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
            htmlFor={`edit-url-${provider.id}`}
            className="w-16 shrink-0 text-xs text-muted-foreground"
          >
            Base URL
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
            htmlFor={`edit-priority-${provider.id}`}
            className="w-16 shrink-0 text-xs text-muted-foreground"
          >
            Priority
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
            Enabled
          </label>
          <div className="ml-auto flex items-center gap-1">
            <Button
              variant="ghost"
              size="icon"
              className="size-7 text-primary hover:text-primary"
              onClick={handleSaveEdit}
              disabled={saving}
              aria-label="Save changes"
            >
              <Check className="size-3.5" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className="size-7 text-muted-foreground hover:text-foreground"
              onClick={handleCancelEdit}
              disabled={saving}
              aria-label="Cancel editing"
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
        aria-label="Drag to reorder"
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
          {provider.kind} · priority {provider.priority}
        </p>
      </div>

      <span
        className={cn(
          "text-xs tabular-nums",
          health?.success_rate != null && health.success_rate < 0.9
            ? "text-yellow-600"
            : "text-muted-foreground",
        )}
        title="Success rate"
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
          aria-label={`${provider.enabled ? "Disable" : "Enable"} ${provider.name}`}
        />
      </label>

      <Button
        variant="ghost"
        size="icon"
        className="size-7 text-muted-foreground hover:text-foreground"
        onClick={handleStartEdit}
        aria-label={`Edit ${provider.name}`}
      >
        <Pencil className="size-3.5" />
      </Button>

      <Button
        variant="ghost"
        size="icon"
        className="size-7 text-muted-foreground hover:text-destructive"
        onClick={() => onDelete(provider.id, provider.name)}
        aria-label={`Delete ${provider.name}`}
      >
        <Trash2 className="size-3.5" />
      </Button>
    </div>
  );
}

// ─── Provider List ───────────────────────────────────────────

interface ProviderListProps {
  refreshKey?: number;
}

export function ProviderList({ refreshKey }: ProviderListProps) {
  const [providers, setProviders] = useState<Provider[]>([]);
  const [healthMap, setHealthMap] = useState<Record<string, HealthSnapshot>>(
    {},
  );
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
              name: p.name,
              kind: p.kind,
              base_url: p.base_url,
              api_key: "",
              priority: newPriority,
              enabled: p.enabled,
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
      const input: ProviderInput = {
        name: provider.name,
        kind: provider.kind,
        base_url: provider.base_url,
        api_key: "",
        priority: provider.priority,
        enabled,
      };
      try {
        const updated = await updateProvider(id, input);
        setProviders((prev) => prev.map((p) => (p.id === id ? updated : p)));
      } catch (err: unknown) {
        setError(err instanceof Error ? err.message : String(err));
      }
    },
    [providers],
  );

  const handleSaveEdit = useCallback(
    async (id: string, patch: Partial<EditState>) => {
      const provider = providers.find((p) => p.id === id);
      if (!provider) return;
      const input: ProviderInput = {
        name: patch.name ?? provider.name,
        kind: provider.kind,
        base_url: patch.base_url ?? provider.base_url,
        api_key: "",
        priority: patch.priority ? Number(patch.priority) : provider.priority,
        enabled: patch.enabled ?? provider.enabled,
      };
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
        No providers yet. Add one below.
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
                onDelete={handleRequestDelete}
                onToggleEnabled={handleToggleEnabled}
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
