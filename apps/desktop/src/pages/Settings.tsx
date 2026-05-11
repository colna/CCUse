import { useCallback } from "react";
import { useTranslation } from "react-i18next";

import { ConfigExportPanel } from "@/components/settings/ConfigExportPanel";

const I18N_STORAGE_KEY = "i18nextLng";

/**
 * 设置页：当前只放语言切换和配置导入导出。
 *
 * 语言下拉的"跟随系统"是手动清掉 localStorage 里 i18next 缓存的语言，
 * 让 i18next-browser-languagedetector 下次启动重新走 navigator
 * 检测；同时立即切到一个合理值，避免下拉显示与实际语言不一致。
 */
export function SettingsPage() {
  const { t, i18n } = useTranslation("monitor");
  const { t: tc } = useTranslation("common");

  const handleLanguageChange = useCallback(
    (e: React.ChangeEvent<HTMLSelectElement>) => {
      const value = e.target.value;
      if (value === "auto") {
        localStorage.removeItem(I18N_STORAGE_KEY);
        i18n.changeLanguage(navigator.language.startsWith("zh") ? "zh" : "en");
      } else {
        i18n.changeLanguage(value);
      }
    },
    [i18n],
  );

  const currentLng = localStorage.getItem(I18N_STORAGE_KEY);
  const selectValue =
    currentLng === "en" || currentLng === "zh" ? currentLng : "auto";

  return (
    <section className="space-y-8">
      <div className="space-y-3">
        <h2 className="text-2xl font-semibold leading-apple-headline tracking-apple-tight">
          {t("settings_title")}
        </h2>
        <p className="text-sm leading-relaxed text-muted-foreground">
          {t("settings_desc")}
        </p>
      </div>

      <div className="space-y-3">
        <h3 className="text-sm font-medium text-foreground">
          {tc("language")}
        </h3>
        <select
          value={selectValue}
          onChange={handleLanguageChange}
          className="rounded-md border border-[var(--app-border)] bg-[var(--app-bg-container)] px-3 py-2 text-sm outline-none focus-visible:border-[var(--app-primary)]"
        >
          <option value="auto">{tc("language_auto")}</option>
          <option value="en">{tc("language_en")}</option>
          <option value="zh">{tc("language_zh")}</option>
        </select>
      </div>

      <ConfigExportPanel />
    </section>
  );
}
