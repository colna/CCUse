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

/**
 * 主导航项。顺序即视觉顺序；i18n key 必须存在于 `common.json` 中。
 */
const NAV_ITEMS: readonly NavItem[] = [
  { to: "/dashboard", labelKey: "nav_dashboard", Icon: AppstoreOutlined },
  { to: "/providers", labelKey: "nav_providers", Icon: CloudServerOutlined },
  { to: "/strategy", labelKey: "nav_strategy", Icon: SwapOutlined },
  { to: "/settings", labelKey: "nav_settings", Icon: SettingOutlined },
];

export function Sidebar() {
  const { t } = useTranslation("common");

  return (
    <nav
      aria-label={t("nav_main")}
      style={{
        background: "var(--app-bg-container)",
        borderRight: "1px solid var(--app-border-secondary)",
        color: "var(--app-text)",
      }}
      className="flex h-full w-60 shrink-0 flex-col px-4 py-7"
    >
      <p
        className="px-3 text-[11px] font-semibold uppercase tracking-[0.22em]"
        style={{ color: "var(--app-text-tertiary)" }}
      >
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
                  isActive ? "is-active" : "is-default",
                )
              }
              style={({ isActive }) =>
                isActive
                  ? {
                      background: "var(--app-primary-bg)",
                      color: "var(--app-primary)",
                    }
                  : { color: "var(--app-text-secondary)" }
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
