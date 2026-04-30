import { getTranslations } from "next-intl/server";
import { notFound } from "next/navigation";

import { DocsContentShell } from "../../../components/docs-content-shell";
import { isLocale } from "../../../i18n/routing";
import { getDocsContent } from "../../../lib/docs-content";
import { getDocsTableOfContents } from "../../../lib/docs";

type DocsIndexPageProps = {
  params: {
    locale: string;
  };
};

export default async function DocsIndexPage({ params }: DocsIndexPageProps) {
  const { locale } = params;

  if (!isLocale(locale)) {
    notFound();
  }

  const t = await getTranslations({ locale, namespace: "Docs" });
  const Content = getDocsContent(locale);
  const tocItems = getDocsTableOfContents(locale);

  return (
    <DocsContentShell tocItems={tocItems} tocLabel={t("tocLabel")}>
      <Content />
    </DocsContentShell>
  );
}
