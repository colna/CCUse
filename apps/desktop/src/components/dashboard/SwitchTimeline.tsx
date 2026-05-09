import { useCallback, useEffect, useState } from "react";
import {
  ArrowRightOutlined,
  CaretDownOutlined,
  CaretRightOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";

import { cn } from "@/lib/utils";
import { getSwitchTimeline, type SwitchEvent } from "@/lib/tauri";

function formatTimestamp(timestamp: string): string {
  const d = new Date(timestamp);
  return d.toLocaleString([], {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

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

interface TimelineRowProps {
  event: SwitchEvent;
}

function TimelineRow({ event }: TimelineRowProps) {
  const [expanded, setExpanded] = useState(false);
  const { t } = useTranslation("monitor");

  return (
    <div className="border-b border-[var(--ant-color-border-secondary,rgba(0,0,0,0.06))] last:border-b-0">
      <button
        type="button"
        onClick={() => setExpanded((e) => !e)}
        className="flex w-full items-center gap-3 px-4 py-3 text-left text-sm transition-colors hover:bg-[var(--ant-color-fill-quaternary,rgba(0,0,0,0.02))]"
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
        <div className="space-y-1 bg-muted/20 px-4 py-3 pl-11 text-xs">
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

const REFRESH_INTERVAL = 15_000;

export function SwitchTimeline() {
  const { t } = useTranslation("monitor");
  const { t: tc } = useTranslation("common");
  const [events, setEvents] = useState<SwitchEvent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback(async () => {
    try {
      const timeline: SwitchEvent[] = await getSwitchTimeline();
      setEvents(timeline);
      setError(null);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  useEffect(() => {
    const id = setInterval(fetchData, REFRESH_INTERVAL);
    return () => clearInterval(id);
  }, [fetchData]);

  if (error) {
    return (
      <div className="rounded-2xl border border-[var(--ant-color-error-border,rgba(255,77,79,0.4))] bg-[var(--ant-color-bg-container,#fff)] p-4 text-sm text-destructive">
        {error}
      </div>
    );
  }

  return (
    <div className="rounded-2xl border border-[var(--ant-color-border-secondary,rgba(0,0,0,0.06))] bg-[var(--ant-color-bg-container,#fff)]">
      <div className="border-b border-[var(--ant-color-border-secondary,rgba(0,0,0,0.06))] px-4 py-3">
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
