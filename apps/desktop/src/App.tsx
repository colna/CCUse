import { Navigate, Route, HashRouter, Routes } from "react-router-dom";

import { AppShell } from "@/components/layout/AppShell";
import { DashboardPage } from "@/pages/Dashboard";
import { ProvidersPage } from "@/pages/Providers";
import { StrategyPage } from "@/pages/Strategy";
import { SettingsPage } from "@/pages/Settings";

/**
 * 路由根。用 HashRouter 是因为 Tauri 把前端打成静态文件加载，HashRouter
 * 不依赖任何服务器端的 history fallback。所有页面共享 `AppShell` 布局。
 */
export default function App() {
  return (
    <HashRouter>
      <Routes>
        <Route element={<AppShell />}>
          <Route index element={<Navigate to="/dashboard" replace />} />
          <Route path="/dashboard" element={<DashboardPage />} />
          <Route path="/providers" element={<ProvidersPage />} />
          <Route path="/strategy" element={<StrategyPage />} />
          <Route path="/settings" element={<SettingsPage />} />
          {/* 任何未注册的 hash 路径回到 dashboard，避免白屏。 */}
          <Route path="*" element={<Navigate to="/dashboard" replace />} />
        </Route>
      </Routes>
    </HashRouter>
  );
}
