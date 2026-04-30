import type { ComponentType } from "react";

import EnFaqTroubleshooting from "../content/docs/en/faq-troubleshooting.mdx";
import EnGettingStarted from "../content/docs/en/getting-started.mdx";
import EnIndex from "../content/docs/en/index.mdx";
import EnMonitoringAlerts from "../content/docs/en/monitoring-alerts.mdx";
import EnSwitchingStrategies from "../content/docs/en/switching-strategies.mdx";
import ZhFaqTroubleshooting from "../content/docs/zh/faq-troubleshooting.mdx";
import ZhGettingStarted from "../content/docs/zh/getting-started.mdx";
import ZhIndex from "../content/docs/zh/index.mdx";
import ZhMonitoringAlerts from "../content/docs/zh/monitoring-alerts.mdx";
import ZhSwitchingStrategies from "../content/docs/zh/switching-strategies.mdx";
import type { Locale } from "../i18n/routing";

type DocsContentComponent = ComponentType<Record<string, never>>;

const docsContent: Record<Locale, Record<string, DocsContentComponent>> = {
  en: {
    "faq-troubleshooting": EnFaqTroubleshooting,
    "getting-started": EnGettingStarted,
    index: EnIndex,
    "monitoring-alerts": EnMonitoringAlerts,
    "switching-strategies": EnSwitchingStrategies,
  },
  zh: {
    "faq-troubleshooting": ZhFaqTroubleshooting,
    "getting-started": ZhGettingStarted,
    index: ZhIndex,
    "monitoring-alerts": ZhMonitoringAlerts,
    "switching-strategies": ZhSwitchingStrategies,
  },
};

export function getDocsContent(locale: Locale, slug = "index") {
  return docsContent[locale][slug || "index"];
}

export function getRegisteredDocsSlugs(locale: Locale) {
  return Object.keys(docsContent[locale]).filter((slug) => slug !== "index");
}
