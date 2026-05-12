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
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { useTranslation } from "react-i18next";

import {
  deleteProvider,
  getHealthSnapshot,
  listProviders,
  onProviderStatusChanged,
  testProviderConnection,
  updateProvider,
  type HealthSnapshot,
  type Provider,
  type ProviderInput,
} from "@/lib/tauri";

import { DeleteDialog, ProviderErrorDialog } from "./dialogs";
import { SortableProviderItem, type EditState } from "./SortableProviderItem";

/**
 * Provider 列表的"容器组件"：
 * - 拉取并维护 `providers` / `healthMap`；
 * - 把 CRUD / 拖拽排序 / 测试连接的副作用整合到 Tauri command；
 * - 复杂展示拆给 `SortableProviderItem`，弹窗拆给 `dialogs.tsx`。
 *
 * 排序策略：仪表盘里"优先级数字越小越优先"。拖拽后我们用每 10 一档的等差
 * 数列重写优先级（10, 20, 30…），这样后续手填的数字 (15 之类) 还能插队。
 */

const HEALTH_REFRESH_INTERVAL_MS = 5_000;
const PRIORITY_STEP = 10;
const PROVIDER_GROUPS = [
  {
    id: "openai",
    titleKey: "provider_group_openai_title",
    descKey: "provider_group_openai_desc",
    kinds: new Set<ProviderInput["kind"]>(["openai", "relay", "custom"]),
  },
  {
    id: "anthropic",
    titleKey: "provider_group_anthropic_title",
    descKey: "provider_group_anthropic_desc",
    kinds: new Set<ProviderInput["kind"]>(["anthropic"]),
  },
  {
    id: "gemini",
    titleKey: "provider_group_gemini_title",
    descKey: "provider_group_gemini_desc",
    kinds: new Set<ProviderInput["kind"]>(["gemini"]),
  },
] as const;

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
      setProviders(await listProviders());
      setError(null);
    } catch (err: unknown) {
      setError(errorMessage(err));
    }
  }, []);

  const fetchHealth = useCallback(async () => {
    // 健康快照拉失败不影响主列表；保持 catch 为空意图明确：背景刷新
    // 失败不该弹错给用户。
    try {
      const snap = await getHealthSnapshot();
      setHealthMap(indexByProvider(snap.providers));
    } catch {
      /* 静默：见上 */
    }
  }, []);

  useEffect(() => {
    fetchProviders();
    fetchHealth();
  }, [fetchProviders, fetchHealth, refreshKey]);

  useEffect(() => {
    const id = setInterval(fetchHealth, HEALTH_REFRESH_INTERVAL_MS);
    return () => clearInterval(id);
  }, [fetchHealth]);

  useEffect(() => {
    // 后端推送状态变化时立刻 patch 当前 row，并触发一次完整刷新拿
    // 到最新的 success_rate / response_time。
    //
    // `.catch(() => undefined)` 是为了消化非 Tauri 环境（测试 / 浏览
    // 器预览）下 `listen()` 没有 `__TAURI_INTERNALS__` 而抛出的
    // unhandled rejection；那种场合下我们就放弃订阅，但页面其余部
    // 分仍然要可用。
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
    }).catch(() => undefined);
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

      // 重写优先级时若任意一次写入失败，立刻 refetch 把 UI 与后端对齐
      // 而不是部分写入半途留下脏状态。
      for (let i = 0; i < reordered.length; i++) {
        const p = reordered[i]!;
        const newPriority = (i + 1) * PRIORITY_STEP;
        if (p.priority === newPriority) continue;
        try {
          await updateProvider(
            p.id,
            providerToInput(p, { priority: newPriority }),
          );
        } catch {
          fetchProviders();
          return;
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
      setError(errorMessage(err));
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
      try {
        const updated = await updateProvider(
          id,
          providerToInput(provider, { enabled }),
        );
        setProviders((prev) => prev.map((p) => (p.id === id ? updated : p)));
      } catch (err: unknown) {
        setError(errorMessage(err));
      }
    },
    [providers],
  );

  const handleTestConnection = useCallback(
    async (id: string) => {
      setTestingIds((current) => ({ ...current, [id]: true }));
      const provider = providers.find((item) => item.id === id);
      const providerName = provider?.name ?? id;
      try {
        const result = await testProviderConnection(id);
        setHealthMap((current) => ({
          ...current,
          [id]: healthSnapshotFromCheckResult(
            id,
            providerName,
            current[id],
            result,
          ),
        }));
        if (!result.success) {
          setTestErrorDialog({
            providerName,
            message: composeCheckErrorMessage(result),
          });
        } else {
          setError(null);
        }
      } catch (err: unknown) {
        setTestErrorDialog({ providerName, message: errorMessage(err) });
      } finally {
        setTestingIds((current) => ({ ...current, [id]: false }));
      }
    },
    [providers],
  );

  const handleSaveEdit = useCallback(
    async (id: string, patch: EditState) => {
      const provider = providers.find((p) => p.id === id);
      if (!provider) return;
      try {
        const updated = await updateProvider(
          id,
          providerToInput(provider, {
            name: patch.name.trim(),
            kind: patch.kind,
            base_url: patch.base_url.trim().replace(/\/$/, ""),
            api_key: patch.api_key.trim(),
            priority: patch.priority
              ? Number(patch.priority)
              : provider.priority,
            enabled: patch.enabled,
          }),
        );
        setProviders((prev) => prev.map((p) => (p.id === id ? updated : p)));
      } catch (err: unknown) {
        setError(errorMessage(err));
      }
    },
    [providers],
  );

  if (error) {
    return (
      <div className="rounded-2xl border border-[var(--app-error-border)] bg-[var(--app-bg-container)] p-4 text-sm text-destructive">
        {error}
      </div>
    );
  }

  if (providers.length === 0) {
    return (
      <div className="rounded-2xl border border-dashed border-[var(--app-border-secondary)] bg-[var(--app-bg-subtle)] px-6 py-10 text-center text-sm text-muted-foreground">
        {t("no_providers")}
      </div>
    );
  }

  const groupedProviders = groupProviders(providers);
  const sortableProviderIds = groupedProviders.flatMap(({ providers }) =>
    providers.map((provider) => provider.id),
  );

  return (
    <>
      <DndContext
        sensors={sensors}
        collisionDetection={closestCenter}
        onDragEnd={handleDragEnd}
      >
        <SortableContext
          items={sortableProviderIds}
          strategy={verticalListSortingStrategy}
        >
          <div className="space-y-5">
            {groupedProviders.map(({ group, providers }) => (
              <section
                key={group.id}
                aria-labelledby={`${group.id}-providers-title`}
              >
                <header className="mb-2.5">
                  <h3
                    id={`${group.id}-providers-title`}
                    className="text-sm font-semibold leading-apple-headline tracking-apple-tight"
                  >
                    {t(group.titleKey)}
                  </h3>
                  <p className="mt-1 text-xs text-muted-foreground">
                    {t(group.descKey)}
                  </p>
                </header>
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
              </section>
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

/** 把 Provider 复刻成可发回 Rust 的 ProviderInput；api_key 默认留空以
 * 复用后端"空串=保留旧密钥"的约定。 */
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

function indexByProvider(
  snapshots: HealthSnapshot[],
): Record<string, HealthSnapshot> {
  const map: Record<string, HealthSnapshot> = {};
  for (const s of snapshots) map[s.provider_id] = s;
  return map;
}

function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

function groupProviders(providers: Provider[]) {
  return PROVIDER_GROUPS.map((group) => ({
    group,
    providers: providers.filter((provider) => group.kinds.has(provider.kind)),
  })).filter((entry) => entry.providers.length > 0);
}

/** 把一次 stream check 的结果折叠回 HealthSnapshot 的形状，方便和
 * 后端 sliding-window 的数据共用一套渲染逻辑。 */
function healthSnapshotFromCheckResult(
  providerId: string,
  providerName: string,
  previous: HealthSnapshot | undefined,
  result: import("@/lib/tauri").StreamCheckResult,
): HealthSnapshot {
  return {
    provider_id: providerId,
    provider_name: providerName,
    status: result.success
      ? result.status === "degraded"
        ? "degraded"
        : "healthy"
      : "down",
    success_rate: result.success ? 1 : 0,
    response_time_us:
      result.response_time_ms != null
        ? result.response_time_ms * 1000
        : (previous?.response_time_us ?? null),
  };
}

/** 把后端的 stream check 错误聚合成多行可读文本，喂给错误弹窗。 */
function composeCheckErrorMessage(
  result: import("@/lib/tauri").StreamCheckResult,
): string {
  return [
    result.message,
    result.http_status != null ? `HTTP ${result.http_status}` : null,
    result.error_category ? `Category: ${result.error_category}` : null,
    result.model_used ? `Model: ${result.model_used}` : null,
  ]
    .filter(Boolean)
    .join("\n");
}
