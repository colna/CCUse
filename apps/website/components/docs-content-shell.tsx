import type { ReactNode } from "react";

import { DocsTableOfContents } from "./docs-table-of-contents";
import type { DocsTocItem } from "../lib/docs";

type DocsContentShellProps = {
  children: ReactNode;
  tocItems: DocsTocItem[];
  tocLabel: string;
};

export function DocsContentShell({
  children,
  tocItems,
  tocLabel,
}: DocsContentShellProps) {
  return (
    <div className="grid gap-8 xl:grid-cols-[minmax(0,1fr)_14rem]">
      <article
        className="min-w-0 rounded-lg border border-border bg-card p-6 shadow-sm"
        data-pagefind-body
      >
        {children}
      </article>
      <DocsTableOfContents items={tocItems} label={tocLabel} />
    </div>
  );
}
