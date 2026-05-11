import { useTranslation } from "react-i18next";

interface TopbarProps {
  title: string;
  description?: string;
}

/**
 * 顶部条：左边是当前页面标题 + 描述（由 `AppShell` 根据路由表驱动），
 * 右边是版本徽章。所有色值都走 CSS variable，跟随 `useColorScheme` 切换。
 */
export function Topbar({ title, description }: TopbarProps) {
  const { t } = useTranslation("common");

  return (
    <header
      style={{
        background: "var(--app-bg-container)",
        borderBottom: "1px solid var(--app-border-secondary)",
        color: "var(--app-text)",
      }}
      className="flex h-16 items-center justify-between px-8"
    >
      <div>
        <h1 className="text-lg font-semibold leading-apple-headline tracking-apple-tight">
          {title}
        </h1>
        {description ? (
          <p
            className="mt-0.5 text-xs leading-snug"
            style={{ color: "var(--app-text-secondary)" }}
          >
            {description}
          </p>
        ) : null}
      </div>
      <span
        className="inline-flex items-center gap-2 rounded-full px-3 py-1 text-[11px] uppercase tracking-[0.18em]"
        style={{
          border: "1px solid var(--app-border-secondary)",
          color: "var(--app-text-tertiary)",
        }}
      >
        <span
          aria-hidden
          className="size-1.5 rounded-full"
          style={{ background: "var(--app-primary)" }}
        />
        {t("version_phase")}
      </span>
    </header>
  );
}
