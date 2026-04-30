import type { ComponentType } from "react";

import EnGettingStarted from "../content/docs/en/getting-started.mdx";
import EnIndex from "../content/docs/en/index.mdx";
import EnSwitchingStrategies from "../content/docs/en/switching-strategies.mdx";
import ZhGettingStarted from "../content/docs/zh/getting-started.mdx";
import ZhIndex from "../content/docs/zh/index.mdx";
import ZhSwitchingStrategies from "../content/docs/zh/switching-strategies.mdx";
import type { Locale } from "../i18n/routing";

type DocsContentComponent = ComponentType<Record<string, never>>;

const docsContent: Record<Locale, Record<string, DocsContentComponent>> = {
  en: {
    "getting-started": EnGettingStarted,
    index: EnIndex,
    "switching-strategies": EnSwitchingStrategies,
  },
  zh: {
    "getting-started": ZhGettingStarted,
    index: ZhIndex,
    "switching-strategies": ZhSwitchingStrategies,
  },
};

export function getDocsContent(locale: Locale, slug = "index") {
  return docsContent[locale][slug || "index"];
}

export function getRegisteredDocsSlugs(locale: Locale) {
  return Object.keys(docsContent[locale]).filter((slug) => slug !== "index");
}
