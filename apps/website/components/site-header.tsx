import Image from "next/image";
import { getTranslations } from "next-intl/server";

import { locales, type Locale } from "../i18n/routing";
import { ThemeToggle } from "./theme-toggle";

type SiteHeaderProps = {
  locale: Locale;
};

const navItems = [
  { key: "home", href: "" },
  { key: "features", href: "/features" },
  { key: "docs", href: "/docs" },
  { key: "download", href: "/download" },
] as const;

export async function SiteHeader({ locale }: SiteHeaderProps) {
  const t = await getTranslations({ locale, namespace: "Navigation" });

  return (
    <header className="sticky top-0 z-50 border-b border-border/60 bg-background/90 backdrop-blur-xl">
      <div className="mx-auto flex min-h-16 max-w-5xl flex-wrap items-center justify-between gap-x-6 gap-y-3 px-6 py-3">
        <a
          aria-label={t("brandLabel")}
          className="flex items-center gap-3 text-sm font-semibold text-foreground"
          href={`/${locale}`}
        >
          <Image
            alt=""
            className="h-8 w-8 rounded-lg"
            height={32}
            priority
            src="/icon.png"
            width={32}
          />
          <span>CCUse</span>
        </a>

        <nav
          aria-label={t("primaryLabel")}
          className="order-3 flex w-full items-center gap-4 text-sm text-muted-foreground sm:order-none sm:w-auto"
        >
          {navItems.map((item) => (
            <a
              className="transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
              href={`/${locale}${item.href}`}
              key={item.key}
            >
              {t(`items.${item.key}`)}
            </a>
          ))}
        </nav>

        <div className="flex flex-wrap items-center justify-end gap-3">
          <ThemeToggle
            ariaLabel={t("themeLabel")}
            labels={{
              system: t("themes.system"),
              light: t("themes.light"),
              dark: t("themes.dark"),
            }}
          />

          <div
            aria-label={t("languageLabel")}
            className="flex rounded-md border border-border bg-muted/50 p-0.5 text-xs font-medium"
            role="group"
          >
            {locales.map((item) => {
              const active = item === locale;

              return (
                <a
                  aria-current={active ? "page" : undefined}
                  className={
                    active
                      ? "rounded bg-background px-2.5 py-1 text-foreground shadow-sm"
                      : "rounded px-2.5 py-1 text-muted-foreground transition-colors hover:text-foreground"
                  }
                  href={`/${item}`}
                  key={item}
                  lang={item}
                >
                  {t(`languages.${item}`)}
                </a>
              );
            })}
          </div>

          <a
            className="text-sm font-medium text-primary transition-colors hover:text-primary/80 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
            href="https://github.com/colna/CCUse"
          >
            GitHub
          </a>
        </div>
      </div>
    </header>
  );
}
