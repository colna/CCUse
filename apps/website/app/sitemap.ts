import type { MetadataRoute } from "next";

import { locales } from "../i18n/routing";
import { absoluteUrl } from "../site";

export default function sitemap(): MetadataRoute.Sitemap {
  return locales.map((locale) => ({
    url: absoluteUrl(`/${locale}`),
    lastModified: new Date("2026-04-30T00:00:00.000Z"),
    changeFrequency: "weekly",
    priority: 1,
  }));
}
