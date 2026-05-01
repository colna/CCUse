import { useTranslation } from "react-i18next";

import { ModelMappingTable } from "@/components/model-mapping/ModelMappingTable";
import { ConfigExportPanel } from "@/components/settings/ConfigExportPanel";

export function SettingsPage() {
  const { t, i18n } = useTranslation("monitor");
  const { t: tc } = useTranslation("common");

  const handleLanguageChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    const value = e.target.value;
    if (value === "auto") {
      localStorage.removeItem("i18nextLng");
      const browserLang = navigator.language.startsWith("zh") ? "zh" : "en";
      i18n.changeLanguage(browserLang);
    } else {
      i18n.changeLanguage(value);
    }
  };

  const currentLng = localStorage.getItem("i18nextLng");
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
          className="rounded-md border border-border bg-background px-3 py-2 text-sm outline-none focus-visible:border-primary"
        >
          <option value="auto">{tc("language_auto")}</option>
          <option value="en">{tc("language_en")}</option>
          <option value="zh">{tc("language_zh")}</option>
        </select>
      </div>

      <div className="space-y-3 rounded-lg border border-border bg-card p-4">
        <h3 className="text-sm font-medium text-foreground">
          {t("about_title")}
        </h3>
        <p className="text-sm leading-relaxed text-muted-foreground">
          {t("about_desc")}
        </p>
        <div className="flex flex-wrap gap-3">
          <a
            className="inline-flex h-9 items-center rounded-md bg-primary px-3 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
            href="https://ccuse.app"
            rel="noreferrer"
            target="_blank"
          >
            {t("about_website")}
          </a>
          <a
            className="inline-flex h-9 items-center rounded-md border border-border px-3 text-sm font-medium text-foreground transition-colors hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
            href="https://ccuse.app/download"
            rel="noreferrer"
            target="_blank"
          >
            {t("about_download")}
          </a>
        </div>
      </div>

      <ModelMappingTable />

      <hr className="border-border" />

      <ConfigExportPanel />
    </section>
  );
}
