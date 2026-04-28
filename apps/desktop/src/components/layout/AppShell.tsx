import { Outlet, useLocation } from "react-router-dom";

import { Sidebar } from "./Sidebar";
import { Topbar } from "./Topbar";

interface PageMeta {
  title: string;
  description?: string;
}

const PAGE_META: Record<string, PageMeta> = {
  "/dashboard": { title: "总览", description: "本地代理状态与最近活动" },
  "/providers": {
    title: "供应商",
    description: "管理上游 API 与优先级",
  },
  "/strategy": {
    title: "策略",
    description: "选择切换策略与调整高级参数",
  },
  "/settings": { title: "设置", description: "应用偏好与高级选项" },
};

const FALLBACK_META: PageMeta = {
  title: "CCUse",
  description: "本地 API 代理 + 多供应商无感切换",
};

function metaForPath(pathname: string): PageMeta {
  return PAGE_META[pathname] ?? FALLBACK_META;
}

export function AppShell() {
  const location = useLocation();
  const meta = metaForPath(location.pathname);

  return (
    <div className="flex h-screen min-h-screen w-full bg-background text-foreground">
      <Sidebar />
      <div className="flex flex-1 flex-col">
        <Topbar title={meta.title} description={meta.description} />
        <main className="flex-1 overflow-y-auto px-8 py-6">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
