import Image from "next/image";
import { getTranslations } from "next-intl/server";

import type { Locale } from "../i18n/routing";

type SiteFooterProps = {
  locale: Locale;
};

const footerLinks = [
  { key: "home", href: "" },
  { key: "docs", href: "/docs" },
  { key: "download", href: "/download" },
  { key: "preview", href: "/download/preview" },
  { key: "privacy", href: "/legal/privacy" },
  { key: "terms", href: "/legal/terms" },
  { key: "github", href: "https://github.com/colna/CCUse" },
] as const;

export async function SiteFooter({ locale }: SiteFooterProps) {
  const t = await getTranslations({ locale, namespace: "Footer" });

  return (
    <footer className="border-t border-border bg-muted/40">
      <div className="mx-auto grid max-w-5xl gap-8 px-6 py-10 sm:grid-cols-[1fr_auto]">
        <div>
          <a
            aria-label={t("brandLabel")}
            className="inline-flex items-center gap-3 text-sm font-semibold text-foreground"
            href={`/${locale}`}
          >
            <Image
              alt=""
              className="h-8 w-8 rounded-lg"
              height={32}
              src="/icon.png"
              width={32}
            />
            <span>CCUse</span>
          </a>
          <p className="mt-4 max-w-md text-sm leading-6 text-muted-foreground">
            {t("tagline")}
          </p>
        </div>

        <nav
          aria-label={t("secondaryLabel")}
          className="grid gap-3 text-sm text-muted-foreground sm:justify-items-end"
        >
          {footerLinks.map((item) => {
            const href = item.href.startsWith("http")
              ? item.href
              : `/${locale}${item.href}`;

            return (
              <a
                className="transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                href={href}
                key={item.key}
              >
                {t(`links.${item.key}`)}
              </a>
            );
          })}
        </nav>

        <p className="text-xs text-muted-foreground sm:col-span-2">
          {t("copyright")}
        </p>
      </div>
    </footer>
  );
}
