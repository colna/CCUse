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
import { GripVertical, Trash2 } from "lucide-react";

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

/** Health status dot color. */
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

interface SortableProviderItemProps {
  provider: Provider;
  health?: HealthSnapshot;
  onDelete: (id: string) => void;
  onToggleEnabled: (id: string, enabled: boolean) => void;
}

function SortableProviderItem({
  provider,
  health,
  onDelete,
  onToggleEnabled,
}: SortableProviderItemProps) {
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
        aria-label="拖拽排序"
      >
        <GripVertical className="size-4" />
      </button>

      <span
        className={cn("size-2.5 shrink-0 rounded-full", statusColor(health?.status))}
        title={health?.status ?? "unknown"}
      />

      <div className="min-w-0 flex-1">
        <p className="truncate text-sm font-medium">{provider.name}</p>
        <p className="truncate text-xs text-muted-foreground">
          {provider.kind} · 优先级 {provider.priority}
        </p>
      </div>

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
          aria-label={`${provider.enabled ? "禁用" : "启用"} ${provider.name}`}
        />
      </label>

      <Button
        variant="ghost"
        size="icon"
        className="size-7 text-muted-foreground hover:text-destructive"
        onClick={() => onDelete(provider.id)}
        aria-label={`删除 ${provider.name}`}
      >
        <Trash2 className="size-3.5" />
      </Button>
    </div>
  );
}

interface ProviderListProps {
  refreshKey?: number;
}

export function ProviderList({ refreshKey }: ProviderListProps) {
  const [providers, setProviders] = useState<Provider[]>([]);
  const [healthMap, setHealthMap] = useState<Record<string, HealthSnapshot>>({});
  const [error, setError] = useState<string | null>(null);

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
      // Health snapshot not critical — ignore
    }
  }, []);

  useEffect(() => {
    fetchProviders();
    fetchHealth();
  }, [fetchProviders, fetchHealth, refreshKey]);

  // Refresh health every 5s
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

      // Optimistic UI update
      setProviders(reordered);

      // Update priorities based on new order (10, 20, 30, ...)
      for (let i = 0; i < reordered.length; i++) {
        const p = reordered[i];
        const newPriority = (i + 1) * 10;
        if (p.priority !== newPriority) {
          try {
            await updateProvider(p.id, {
              name: p.name,
              kind: p.kind,
              base_url: p.base_url,
              api_key: "", // backend keeps existing key if empty
              priority: newPriority,
              enabled: p.enabled,
            });
          } catch {
            // Revert on error
            fetchProviders();
            return;
          }
        }
      }
      fetchProviders();
    },
    [providers, fetchProviders],
  );

  const handleDelete = useCallback(
    async (id: string) => {
      try {
        await deleteProvider(id);
        setProviders((prev) => prev.filter((p) => p.id !== id));
      } catch (err: unknown) {
        setError(err instanceof Error ? err.message : String(err));
      }
    },
    [],
  );

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
        setProviders((prev) =>
          prev.map((p) => (p.id === id ? updated : p)),
        );
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
        暂无供应商。请在下方添加。
      </div>
    );
  }

  return (
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
              onDelete={handleDelete}
              onToggleEnabled={handleToggleEnabled}
            />
          ))}
        </div>
      </SortableContext>
    </DndContext>
  );
}
