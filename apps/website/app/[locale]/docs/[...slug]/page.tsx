import { getTranslations } from "next-intl/server";
import { notFound } from "next/navigation";

import { DocsContentShell } from "../../../../components/docs-content-shell";
import { isLocale, locales } from "../../../../i18n/routing";
import { getDocsTableOfContents } from "../../../../lib/docs";
import {
  getDocsContent,
  getRegisteredDocsSlugs,
} from "../../../../lib/docs-content";

type DocsPageProps = {
  params: {
    locale: string;
    slug: string[];
  };
};

export function generateStaticParams() {
  return locales.flatMap((locale) =>
    getRegisteredDocsSlugs(locale).map((slug) => ({
      locale,
      slug: slug.split("/"),
    })),
  );
}

export default async function DocsPage({ params }: DocsPageProps) {
  const { locale, slug } = params;

  if (!isLocale(locale)) {
    notFound();
  }

  const normalizedSlug = slug.join("/");
  const Content = getDocsContent(locale, normalizedSlug);

  if (!Content) {
    notFound();
  }

  const t = await getTranslations({ locale, namespace: "Docs" });
  const tocItems = getDocsTableOfContents(locale, normalizedSlug);

  return (
    <DocsContentShell tocItems={tocItems} tocLabel={t("tocLabel")}>
      <Content />
    </DocsContentShell>
  );
}
