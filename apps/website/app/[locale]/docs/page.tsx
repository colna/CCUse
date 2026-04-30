import { notFound } from "next/navigation";

import EnDocsIndex from "../../../content/docs/en/index.mdx";
import ZhDocsIndex from "../../../content/docs/zh/index.mdx";
import { isLocale } from "../../../i18n/routing";

type DocsIndexPageProps = {
  params: {
    locale: string;
  };
};

export default function DocsIndexPage({ params }: DocsIndexPageProps) {
  const { locale } = params;

  if (!isLocale(locale)) {
    notFound();
  }

  const Content = locale === "zh" ? ZhDocsIndex : EnDocsIndex;

  return (
    <article className="mx-auto max-w-3xl rounded-lg border border-border bg-card p-6 shadow-sm">
      <Content />
    </article>
  );
}
