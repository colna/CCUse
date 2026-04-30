import { getTranslations } from "next-intl/server";
import { notFound } from "next/navigation";

import { DocsTableOfContents } from "../../../components/docs-table-of-contents";
import EnDocsIndex from "../../../content/docs/en/index.mdx";
import ZhDocsIndex from "../../../content/docs/zh/index.mdx";
import { isLocale } from "../../../i18n/routing";
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
  const Content = locale === "zh" ? ZhDocsIndex : EnDocsIndex;
  const tocItems = getDocsTableOfContents(locale);

  return (
    <div className="grid gap-8 xl:grid-cols-[minmax(0,1fr)_14rem]">
      <article className="min-w-0 rounded-lg border border-border bg-card p-6 shadow-sm">
        <Content />
      </article>
      <DocsTableOfContents items={tocItems} label={t("tocLabel")} />
    </div>
  );
}
