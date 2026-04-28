import {
  LayoutDashboard,
  Server,
  Shuffle,
  Settings as SettingsIcon,
} from "lucide-react";
import { NavLink } from "react-router-dom";
import { useTranslation } from "react-i18next";

import { cn } from "@/lib/utils";

interface NavItem {
  to: string;
  labelKey: string;
  icon: typeof LayoutDashboard;
}

const NAV_ITEMS: readonly NavItem[] = [
  { to: "/dashboard", labelKey: "nav_dashboard", icon: LayoutDashboard },
  { to: "/providers", labelKey: "nav_providers", icon: Server },
  { to: "/strategy", labelKey: "nav_strategy", icon: Shuffle },
  { to: "/settings", labelKey: "nav_settings", icon: SettingsIcon },
] as const;

export function Sidebar() {
  const { t } = useTranslation("common");

  return (
    <nav
      aria-label={t("nav_dashboard")}
      className="flex h-full w-56 shrink-0 flex-col border-r border-border bg-card/40 px-3 py-6"
    >
      <p className="px-3 text-xs uppercase tracking-[0.18em] text-muted-foreground">
        {t("brand")}
      </p>
      <ul className="mt-6 space-y-1">
        {NAV_ITEMS.map(({ to, labelKey, icon: Icon }) => (
          <li key={to}>
            <NavLink
              to={to}
              className={({ isActive }) =>
                cn(
                  "flex items-center gap-3 rounded-md px-3 py-2 text-sm leading-snug tracking-apple-tight transition-colors",
                  isActive
                    ? "bg-primary/10 text-primary"
                    : "text-foreground/80 hover:bg-muted hover:text-foreground",
                )
              }
            >
              <Icon className="size-4" aria-hidden />
              <span>{t(labelKey)}</span>
            </NavLink>
          </li>
        ))}
      </ul>
    </nav>
  );
}
