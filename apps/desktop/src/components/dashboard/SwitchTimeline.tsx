import {
  ArrowRightOutlined,
  CaretDownOutlined,
  CaretRightOutlined,
} from "@ant-design/icons";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { cn } from "@/lib/utils";
import { getSwitchTimeline, type SwitchEvent } from "@/lib/tauri";
import { useTimeseriesPoll } from "@/lib/useTimeseriesPoll";

/**
 * 后端切换历史时间线。每条记录可点击展开，展示触发原因、详情、ID。
 * 数据后端默认按时间倒序返回。
 */

const REFRESH_INTERVAL_MS = 15_000;

/** 策略标签的配色 —— 视觉上分组的"哪类策略导致切换"。 */
function strategyBadgeColor(strategy: string): string {
  switch (strategy) {
    case "priority":
      return "bg-blue-100 text-blue-700";
    case "fastest":
      return "bg-green-100 text-green-700";
    case "cost":
      return "bg-amber-100 text-amber-700";
    case "load_balance":
      return "bg-purple-100 text-purple-700";
    case "smart":
      return "bg-pink-100 text-pink-700";
    default:
      return "bg-muted text-muted-foreground";
  }
}

function formatTimestamp(timestamp: string): string {
  return new Date(timestamp).toLocaleString([], {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

export function SwitchTimeline() {
  const { t } = useTranslation("monitor");
  const { t: tc } = useTranslation("common");
  const { data, loading, error } = useTimeseriesPoll(
    getSwitchTimeline,
    REFRESH_INTERVAL_MS,
  );
  const events = data ?? [];

  if (error) {
    return (
      <div className="rounded-2xl border border-[var(--app-error-border)] bg-[var(--app-bg-container)] p-4 text-sm text-destructive">
        {error}
      </div>
    );
  }

  return (
    <div className="rounded-2xl border border-[var(--app-border-secondary)] bg-[var(--app-bg-container)]">
      <div className="border-b border-[var(--app-border-secondary)] px-4 py-3">
        <h4 className="text-sm font-medium">{t("switch_timeline_title")}</h4>
      </div>
      {loading ? (
        <div className="flex h-32 items-center justify-center text-sm text-muted-foreground">
          {tc("loading")}
        </div>
      ) : events.length === 0 ? (
        <div className="px-6 py-8 text-center text-sm text-muted-foreground">
          {t("switch_no_events")}
        </div>
      ) : (
        <div className="max-h-80 overflow-y-auto">
          {events.map((event) => (
            <TimelineRow key={event.id} event={event} />
          ))}
        </div>
      )}
    </div>
  );
}

function TimelineRow({ event }: { event: SwitchEvent }) {
  const [expanded, setExpanded] = useState(false);
  const { t } = useTranslation("monitor");

  return (
    <div className="border-b border-[var(--app-border-secondary)] last:border-b-0">
      <button
        type="button"
        onClick={() => setExpanded((e) => !e)}
        className="flex w-full items-center gap-3 px-4 py-3 text-left text-sm transition-colors hover:bg-[var(--app-bg-subtle)]"
      >
        {expanded ? (
          <CaretDownOutlined
            className="shrink-0 text-xs text-muted-foreground"
            aria-label=""
            role="presentation"
          />
        ) : (
          <CaretRightOutlined
            className="shrink-0 text-xs text-muted-foreground"
            aria-label=""
            role="presentation"
          />
        )}

        <span className="w-36 shrink-0 text-xs tabular-nums text-muted-foreground">
          {formatTimestamp(event.timestamp)}
        </span>

        <span className="flex items-center gap-1 text-xs font-medium">
          <span className="max-w-[80px] truncate" title={event.from_provider}>
            {event.from_provider}
          </span>
          <ArrowRightOutlined
            className="shrink-0 text-[10px] text-muted-foreground"
            aria-label=""
            role="presentation"
          />
          <span className="max-w-[80px] truncate" title={event.to_provider}>
            {event.to_provider}
          </span>
        </span>

        <span
          className={cn(
            "ml-auto shrink-0 rounded-full px-2 py-0.5 text-[10px] font-medium",
            strategyBadgeColor(event.strategy),
          )}
        >
          {event.strategy}
        </span>
      </button>

      {expanded && (
        <div className="bg-muted/20 space-y-1 px-4 py-3 pl-11 text-xs">
          <p>
            <span className="text-muted-foreground">{t("switch_reason")}</span>
            {event.reason}
          </p>
          {event.details && (
            <p>
              <span className="text-muted-foreground">
                {t("switch_details")}
              </span>
              {event.details}
            </p>
          )}
          <p>
            <span className="text-muted-foreground">{t("switch_id")}</span>
            <span className="font-mono">{event.id}</span>
          </p>
        </div>
      )}
    </div>
  );
}
