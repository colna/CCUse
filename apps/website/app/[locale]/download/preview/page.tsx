import type { Metadata } from "next";
import { getTranslations, setRequestLocale } from "next-intl/server";
import { notFound } from "next/navigation";

import { defaultLocale, isLocale, locales } from "../../../../i18n/routing";
import { absoluteUrl, siteName } from "../../../../site";

type PreviewPageProps = {
  params: {
    locale: string;
  };
};

export async function generateMetadata({
  params,
}: PreviewPageProps): Promise<Metadata> {
  const locale = isLocale(params.locale) ? params.locale : defaultLocale;
  const t = await getTranslations({ locale, namespace: "DownloadPreviewPage" });

  return {
    title: t("metadata.title"),
    description: t("metadata.description"),
    alternates: {
      canonical: `/${locale}/download/preview`,
      languages: Object.fromEntries(
        locales.map((item) => [item, `/${item}/download/preview`]),
      ),
    },
    openGraph: {
      title: `${t("metadata.title")} | ${siteName}`,
      description: t("metadata.description"),
      url: absoluteUrl(`/${locale}/download/preview`),
      siteName,
      type: "website",
      locale: locale === "zh" ? "zh_CN" : "en_US",
      alternateLocale: locale === "zh" ? ["en_US"] : ["zh_CN"],
      images: [absoluteUrl("/opengraph-image.png")],
    },
  };
}

export default async function DownloadPreviewPage({
  params,
}: PreviewPageProps) {
  const { locale } = params;

  if (!isLocale(locale)) {
    notFound();
  }

  setRequestLocale(locale);
  const t = await getTranslations({ locale, namespace: "DownloadPreviewPage" });

  return (
    <main className="mx-auto flex min-h-[60vh] max-w-3xl items-center px-6 py-20">
      <section className="space-y-4">
        <p className="text-sm font-semibold text-primary">{t("eyebrow")}</p>
        <h1 className="font-display text-4xl font-semibold leading-apple-headline">
          {t("title")}
        </h1>
        <p className="text-base leading-7 text-muted-foreground">
          {t("description")}
        </p>
        <div className="rounded-lg border border-border bg-muted/40 p-4 text-sm leading-6 text-muted-foreground">
          {t("body")}
        </div>
      </section>
    </main>
  );
}
