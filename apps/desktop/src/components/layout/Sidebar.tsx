import {
  LayoutDashboard,
  Server,
  Shuffle,
  Settings as SettingsIcon,
} from "lucide-react";
import { NavLink } from "react-router-dom";

import { cn } from "@/lib/utils";

interface NavItem {
  to: string;
  label: string;
  icon: typeof LayoutDashboard;
}

const NAV_ITEMS: readonly NavItem[] = [
  { to: "/dashboard", label: "总览", icon: LayoutDashboard },
  { to: "/providers", label: "供应商", icon: Server },
  { to: "/strategy", label: "策略", icon: Shuffle },
  { to: "/settings", label: "设置", icon: SettingsIcon },
] as const;

export function Sidebar() {
  return (
    <nav
      aria-label="主导航"
      className="flex h-full w-56 shrink-0 flex-col border-r border-border bg-card/40 px-3 py-6"
    >
      <p className="px-3 text-xs uppercase tracking-[0.18em] text-muted-foreground">
        CCUse
      </p>
      <ul className="mt-6 space-y-1">
        {NAV_ITEMS.map(({ to, label, icon: Icon }) => (
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
              <span>{label}</span>
            </NavLink>
          </li>
        ))}
      </ul>
    </nav>
  );
}
