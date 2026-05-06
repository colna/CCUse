import type { Metadata } from "next";
import { getTranslations, setRequestLocale } from "next-intl/server";
import { notFound } from "next/navigation";

import { defaultLocale, isLocale, locales } from "../../../../i18n/routing";
import { absoluteUrl, siteName } from "../../../../site";

type LegalPageProps = {
  params: {
    locale: string;
  };
};

export async function generateMetadata({
  params,
}: LegalPageProps): Promise<Metadata> {
  const locale = isLocale(params.locale) ? params.locale : defaultLocale;
  const t = await getTranslations({ locale, namespace: "PrivacyPage" });

  return {
    title: t("metadata.title"),
    description: t("metadata.description"),
    alternates: {
      canonical: `/${locale}/legal/privacy`,
      languages: Object.fromEntries(
        locales.map((item) => [item, `/${item}/legal/privacy`]),
      ),
    },
    openGraph: {
      title: `${t("metadata.title")} | ${siteName}`,
      description: t("metadata.description"),
      url: absoluteUrl(`/${locale}/legal/privacy`),
      siteName,
      type: "website",
      locale: locale === "zh" ? "zh_CN" : "en_US",
      alternateLocale: locale === "zh" ? ["en_US"] : ["zh_CN"],
      images: [absoluteUrl("/opengraph-image.png")],
    },
  };
}

export default async function PrivacyPage({ params }: LegalPageProps) {
  const { locale } = params;

  if (!isLocale(locale)) {
    notFound();
  }

  setRequestLocale(locale);
  const t = await getTranslations({ locale, namespace: "PrivacyPage" });

  return (
    <main className="mx-auto max-w-3xl px-6 py-16">
      <article className="space-y-6">
        <header className="space-y-3">
          <p className="text-sm font-semibold text-primary">{t("eyebrow")}</p>
          <h1 className="font-display text-4xl font-semibold leading-apple-headline">
            {t("title")}
          </h1>
          <p className="text-base leading-7 text-muted-foreground">
            {t("description")}
          </p>
        </header>
        <section className="space-y-4 rounded-lg border border-border bg-card p-6 text-sm leading-7">
          <p>{t("body.intro")}</p>
          <p>{t("body.data")}</p>
          <p>{t("body.contact")}</p>
        </section>
      </article>
    </main>
  );
}
