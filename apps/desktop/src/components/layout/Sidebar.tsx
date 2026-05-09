import {
  AppstoreOutlined,
  CloudServerOutlined,
  SettingOutlined,
  SwapOutlined,
} from "@ant-design/icons";
import type { ComponentType } from "react";
import { NavLink } from "react-router-dom";
import { useTranslation } from "react-i18next";

import { cn } from "@/lib/utils";

interface NavItem {
  to: string;
  labelKey: string;
  Icon: ComponentType<{ className?: string }>;
}

const NAV_ITEMS: readonly NavItem[] = [
  { to: "/dashboard", labelKey: "nav_dashboard", Icon: AppstoreOutlined },
  { to: "/providers", labelKey: "nav_providers", Icon: CloudServerOutlined },
  { to: "/strategy", labelKey: "nav_strategy", Icon: SwapOutlined },
  { to: "/settings", labelKey: "nav_settings", Icon: SettingOutlined },
] as const;

export function Sidebar() {
  const { t } = useTranslation("common");

  return (
    <nav
      aria-label={t("nav_main")}
      className="bg-[var(--ant-color-bg-container,#fff)]/60 flex h-full w-60 shrink-0 flex-col border-r border-[var(--ant-color-border-secondary,rgba(0,0,0,0.06))] px-4 py-7 backdrop-blur"
    >
      <p className="px-3 text-[11px] uppercase tracking-[0.22em] text-muted-foreground">
        {t("brand")}
      </p>
      <ul className="mt-7 space-y-1">
        {NAV_ITEMS.map(({ to, labelKey, Icon }) => (
          <li key={to}>
            <NavLink
              to={to}
              className={({ isActive }) =>
                cn(
                  "flex items-center gap-3 rounded-xl px-3 py-2.5 text-sm leading-snug tracking-apple-tight transition-colors",
                  isActive
                    ? "bg-[var(--ant-color-primary-bg,rgba(0,113,227,0.08))] text-[var(--ant-color-primary,#0071e3)]"
                    : "text-foreground/80 hover:bg-[var(--ant-color-fill-quaternary,rgba(0,0,0,0.04))] hover:text-foreground",
                )
              }
            >
              <Icon className="text-base" />
              <span>{t(labelKey)}</span>
            </NavLink>
          </li>
        ))}
      </ul>
    </nav>
  );
}
