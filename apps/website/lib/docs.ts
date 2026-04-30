import { readdirSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import matter from "gray-matter";

import type { Locale } from "../i18n/routing";

export type DocsNavigationGroup = {
  section: string;
  items: DocsNavigationItem[];
};

export type DocsNavigationItem = {
  description: string;
  href: string;
  order: number;
  section: string;
  slug: string;
  title: string;
};

type DocsFrontmatter = {
  description?: string;
  order?: number;
  section?: string;
  title?: string;
};

const docsRoot = path.join(process.cwd(), "content/docs");

export function getDocsNavigation(locale: Locale): DocsNavigationGroup[] {
  const localeRoot = path.join(docsRoot, locale);
  const docs = collectMdxFiles(localeRoot)
    .map((filePath) => readDocNavigationItem(locale, localeRoot, filePath))
    .sort(
      (left, right) =>
        left.order - right.order || left.title.localeCompare(right.title),
    );

  return docs.reduce<DocsNavigationGroup[]>((groups, item) => {
    const section = item.section;
    const group = groups.find((candidate) => candidate.section === section);

    if (group) {
      group.items.push(item);
      return groups;
    }

    groups.push({ section, items: [item] });
    return groups;
  }, []);
}

function collectMdxFiles(directory: string): string[] {
  return readdirSync(directory).flatMap((entry) => {
    const filePath = path.join(directory, entry);
    const stats = statSync(filePath);

    if (stats.isDirectory()) {
      return collectMdxFiles(filePath);
    }

    return filePath.endsWith(".mdx") ? [filePath] : [];
  });
}

function readDocNavigationItem(
  locale: Locale,
  localeRoot: string,
  filePath: string,
) {
  const raw = readFileSync(filePath, "utf8");
  const parsed = matter(raw);
  const data = parsed.data as DocsFrontmatter;
  const relativePath = path.relative(localeRoot, filePath);
  const slug = relativePath.replace(/\.mdx$/, "").replace(/\/index$/, "");
  const normalizedSlug = slug === "index" ? "" : slug;

  return {
    description: data.description ?? "",
    href: normalizedSlug
      ? `/${locale}/docs/${normalizedSlug}`
      : `/${locale}/docs`,
    order: data.order ?? 100,
    section: data.section ?? "Docs",
    slug: normalizedSlug || "index",
    title: data.title ?? (normalizedSlug || "Docs"),
  };
}
