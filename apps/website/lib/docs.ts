import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import GithubSlugger from "github-slugger";
import matter from "gray-matter";
import { toString } from "mdast-util-to-string";
import remarkParse from "remark-parse";
import { unified } from "unified";

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

export type DocsTocItem = {
  depth: 2 | 3;
  id: string;
  title: string;
};

type DocsFrontmatter = {
  description?: string;
  order?: number;
  section?: string;
  title?: string;
};

type MarkdownNode = {
  children?: MarkdownNode[];
  depth?: number;
  type?: string;
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

export function getDocsTableOfContents(
  locale: Locale,
  slug = "",
): DocsTocItem[] {
  const filePath = resolveDocFilePath(locale, slug);

  if (!filePath) {
    return [];
  }

  const raw = readFileSync(filePath, "utf8");
  const parsed = matter(raw);
  const tree = unified().use(remarkParse).parse(parsed.content) as MarkdownNode;
  const slugger = new GithubSlugger();
  const headings: DocsTocItem[] = [];

  for (const child of tree.children ?? []) {
    if (child.type !== "heading" || (child.depth !== 2 && child.depth !== 3)) {
      continue;
    }

    const title = toString(child as Parameters<typeof toString>[0]).trim();

    if (!title) {
      continue;
    }

    headings.push({
      depth: child.depth,
      id: slugger.slug(title),
      title,
    });
  }

  return headings;
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
  const relativePath = path
    .relative(localeRoot, filePath)
    .split(path.sep)
    .join("/");
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

function resolveDocFilePath(locale: Locale, slug: string) {
  const localeRoot = path.join(docsRoot, locale);
  const normalizedSlug = slug.replace(/^\/+|\/+$/g, "");
  const candidates = normalizedSlug
    ? [
        path.join(localeRoot, `${normalizedSlug}.mdx`),
        path.join(localeRoot, normalizedSlug, "index.mdx"),
      ]
    : [path.join(localeRoot, "index.mdx")];

  return candidates.find(
    (candidate) =>
      candidate.startsWith(localeRoot + path.sep) && existsSync(candidate),
  );
}
