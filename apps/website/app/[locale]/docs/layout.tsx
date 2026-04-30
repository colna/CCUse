import { getTranslations } from "next-intl/server";
import { notFound } from "next/navigation";
import type { ReactNode } from "react";

import { isLocale } from "../../../i18n/routing";
import { getDocsNavigation } from "../../../lib/docs";

type DocsLayoutProps = {
  children: ReactNode;
  params: {
    locale: string;
  };
};

export default async function DocsLayout({
  children,
  params,
}: DocsLayoutProps) {
  const { locale } = params;

  if (!isLocale(locale)) {
    notFound();
  }

  const t = await getTranslations({ locale, namespace: "Docs" });
  const navigationGroups = getDocsNavigation(locale);

  return (
    <main className="bg-background text-foreground">
      <div className="mx-auto grid max-w-6xl gap-8 px-6 py-12 lg:grid-cols-[17rem_1fr]">
        <aside
          aria-label={t("sidebarLabel")}
          className="lg:sticky lg:top-24 lg:self-start"
        >
          <details className="rounded-lg border border-border bg-card p-4" open>
            <summary className="cursor-pointer text-sm font-semibold">
              {t("sidebarTitle")}
            </summary>
            <nav className="mt-4 grid gap-5">
              {navigationGroups.map((group, groupIndex) => (
                <section
                  aria-labelledby={`docs-section-${groupIndex}`}
                  key={group.section}
                >
                  <h2
                    className="text-xs font-semibold uppercase text-muted-foreground"
                    id={`docs-section-${groupIndex}`}
                  >
                    {group.section}
                  </h2>
                  <ul className="mt-2 grid gap-1">
                    {group.items.map((item) => (
                      <li key={item.slug}>
                        <a
                          className="block rounded-md px-3 py-2 text-sm text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                          href={item.href}
                        >
                          <span className="font-medium">{item.title}</span>
                          {item.description ? (
                            <span className="mt-1 block text-xs leading-5 text-muted-foreground">
                              {item.description}
                            </span>
                          ) : null}
                        </a>
                      </li>
                    ))}
                  </ul>
                </section>
              ))}
            </nav>
          </details>
        </aside>
        <div className="min-w-0">{children}</div>
      </div>
    </main>
  );
}
