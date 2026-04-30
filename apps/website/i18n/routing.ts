import { defineRouting } from "next-intl/routing";

export const locales = ["zh", "en"] as const;
export const defaultLocale = "en";

export type Locale = (typeof locales)[number];

export const routing = defineRouting({
  locales,
  defaultLocale,
  localePrefix: "always",
});

export function isLocale(value: string | undefined): value is Locale {
  return locales.some((locale) => locale === value);
}
