import type { ComponentType } from "react";

import EnGettingStarted from "../content/docs/en/getting-started.mdx";
import EnIndex from "../content/docs/en/index.mdx";
import ZhGettingStarted from "../content/docs/zh/getting-started.mdx";
import ZhIndex from "../content/docs/zh/index.mdx";
import type { Locale } from "../i18n/routing";

type DocsContentComponent = ComponentType<Record<string, never>>;

const docsContent: Record<Locale, Record<string, DocsContentComponent>> = {
  en: {
    "getting-started": EnGettingStarted,
    index: EnIndex,
  },
  zh: {
    "getting-started": ZhGettingStarted,
    index: ZhIndex,
  },
};

export function getDocsContent(locale: Locale, slug = "index") {
  return docsContent[locale][slug || "index"];
}

export function getRegisteredDocsSlugs(locale: Locale) {
  return Object.keys(docsContent[locale]).filter((slug) => slug !== "index");
}
