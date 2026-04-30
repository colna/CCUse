import type { MetadataRoute } from "next";

import { locales } from "../i18n/routing";
import { getRegisteredDocsSlugs } from "../lib/docs-content";
import { absoluteUrl } from "../site";

export default function sitemap(): MetadataRoute.Sitemap {
  const lastModified = new Date("2026-04-30T00:00:00.000Z");

  return locales.flatMap((locale) => {
    const docsPages = getRegisteredDocsSlugs(locale).map((slug) => ({
      url: absoluteUrl(`/${locale}/docs/${slug}`),
      lastModified,
      changeFrequency: "weekly" as const,
      priority: 0.7,
    }));

    return [
      {
        url: absoluteUrl(`/${locale}`),
        lastModified,
        changeFrequency: "weekly" as const,
        priority: 1,
      },
      {
        url: absoluteUrl(`/${locale}/docs`),
        lastModified,
        changeFrequency: "weekly" as const,
        priority: 0.8,
      },
      {
        url: absoluteUrl(`/${locale}/features`),
        lastModified,
        changeFrequency: "weekly" as const,
        priority: 0.9,
      },
      {
        url: absoluteUrl(`/${locale}/download`),
        lastModified,
        changeFrequency: "hourly" as const,
        priority: 0.9,
      },
      ...docsPages,
    ];
  });
}
