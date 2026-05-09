import { Outlet, useLocation } from "react-router-dom";
import { useTranslation } from "react-i18next";

import { Sidebar } from "./Sidebar";
import { Topbar } from "./Topbar";

interface PageMeta {
  titleKey: string;
  descKey?: string;
}

const PAGE_META: Record<string, PageMeta> = {
  "/dashboard": {
    titleKey: "page_dashboard_title",
    descKey: "page_dashboard_desc",
  },
  "/providers": {
    titleKey: "page_providers_title",
    descKey: "page_providers_desc",
  },
  "/strategy": {
    titleKey: "page_strategy_title",
    descKey: "page_strategy_desc",
  },
  "/settings": {
    titleKey: "page_settings_title",
    descKey: "page_settings_desc",
  },
};

const FALLBACK_META: PageMeta = {
  titleKey: "page_fallback_title",
  descKey: "page_fallback_desc",
};

function metaForPath(pathname: string): PageMeta {
  return PAGE_META[pathname] ?? FALLBACK_META;
}

export function AppShell() {
  const location = useLocation();
  const meta = metaForPath(location.pathname);
  const { t } = useTranslation("common");

  return (
    <div
      style={{
        background: "var(--app-bg-layout)",
        color: "var(--app-text)",
      }}
      className="flex h-screen min-h-screen w-full"
    >
      <Sidebar />
      <div className="flex flex-1 flex-col">
        <Topbar
          title={t(meta.titleKey)}
          description={meta.descKey ? t(meta.descKey) : undefined}
        />
        <main className="flex-1 overflow-y-auto px-10 py-8">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
