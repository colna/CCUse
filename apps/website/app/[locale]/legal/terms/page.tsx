import { Card, CardContent } from "@ccuse/ui/card";
import type { Metadata } from "next";
import { getTranslations, setRequestLocale } from "next-intl/server";
import { notFound } from "next/navigation";

import { defaultLocale, isLocale, locales } from "../../../../i18n/routing";
import { absoluteUrl, siteName } from "../../../../site";

type TermsPageProps = {
  params: {
    locale: string;
  };
};

export async function generateMetadata({
  params,
}: TermsPageProps): Promise<Metadata> {
  const locale = isLocale(params.locale) ? params.locale : defaultLocale;
  const t = await getTranslations({ locale, namespace: "LegalPage.terms" });
  const canonical = `/${locale}/legal/terms`;
  const languages = Object.fromEntries(
    locales.map((item) => [item, `/${item}/legal/terms`]),
  );

  return {
    title: t("metadata.title"),
    description: t("metadata.description"),
    alternates: {
      canonical,
      languages: {
        ...languages,
        "x-default": `/${defaultLocale}/legal/terms`,
      },
    },
    openGraph: {
      title: `${t("metadata.title")} | ${siteName}`,
      description: t("metadata.description"),
      url: absoluteUrl(canonical),
      siteName,
      type: "website",
    },
  };
}

export default async function TermsPage({ params }: TermsPageProps) {
  const { locale } = params;

  if (!isLocale(locale)) {
    notFound();
  }

  setRequestLocale(locale);
  const t = await getTranslations({ locale, namespace: "LegalPage.terms" });

  return (
    <main className="bg-background text-foreground">
      <section className="border-b border-border bg-background">
        <div className="mx-auto max-w-4xl px-6 py-14 sm:py-16">
          <p className="text-sm font-semibold text-primary">{t("eyebrow")}</p>
          <h1 className="mt-4 font-display text-5xl font-semibold leading-apple-headline">
            {t("title")}
          </h1>
          <p className="mt-5 max-w-2xl text-base leading-7 text-muted-foreground">
            {t("description")}
          </p>
        </div>
      </section>

      <section className="bg-muted/40">
        <div className="mx-auto max-w-4xl px-6 py-12">
          <Card className="border-border/80 bg-card shadow-none">
            <CardContent className="grid gap-8 p-6 sm:p-8">
              {["license", "providers", "previews", "liability"].map(
                (section) => (
                  <section key={section}>
                    <h2 className="font-display text-2xl font-semibold">
                      {t(`sections.${section}.title`)}
                    </h2>
                    <p className="mt-3 text-sm leading-7 text-muted-foreground">
                      {t(`sections.${section}.body`)}
                    </p>
                  </section>
                ),
              )}
            </CardContent>
          </Card>
        </div>
      </section>
    </main>
  );
}
