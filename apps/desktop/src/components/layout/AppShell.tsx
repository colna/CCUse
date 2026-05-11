import { Outlet, useLocation } from "react-router-dom";
import { useTranslation } from "react-i18next";

import { Sidebar } from "./Sidebar";
import { Topbar } from "./Topbar";

interface PageMeta {
  titleKey: string;
  descKey: string;
}

/**
 * 路径 → 页面标题/描述 i18n key 映射。Topbar 文案只与路由相关，
 * 直接表驱动比每个页面各自传 props 更紧凑，也便于和 sidebar 同步翻译。
 */
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

export function AppShell() {
  const { pathname } = useLocation();
  const meta = PAGE_META[pathname] ?? FALLBACK_META;
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
        <Topbar title={t(meta.titleKey)} description={t(meta.descKey)} />
        <main className="flex-1 overflow-y-auto px-10 py-8">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
