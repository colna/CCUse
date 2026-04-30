import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.cwd();

function read(relativePath) {
  return readFileSync(path.join(root, relativePath), "utf8");
}

function readJson(relativePath) {
  return JSON.parse(read(relativePath));
}

function assertPath(relativePath) {
  assert.ok(
    existsSync(path.join(root, relativePath)),
    `${relativePath} must exist`,
  );
}

const packageJson = readJson("package.json");
const docsContent = read("lib/docs-content.ts");
const docsIndexPage = read("app/[locale]/docs/page.tsx");
const docsSlugPage = read("app/[locale]/docs/[...slug]/page.tsx");
const enIndex = read("content/docs/en/index.mdx");
const zhIndex = read("content/docs/zh/index.mdx");
const enGuide = read("content/docs/en/getting-started.mdx");
const zhGuide = read("content/docs/zh/getting-started.mdx");

for (const requiredPath of [
  "app/[locale]/docs/[...slug]/page.tsx",
  "components/docs-content-shell.tsx",
  "content/docs/en/getting-started.mdx",
  "content/docs/zh/getting-started.mdx",
  "lib/docs-content.ts",
]) {
  assertPath(requiredPath);
}

assert.match(packageJson.scripts.test, /verify-docs-getting-started\.mjs/);

assert.match(docsContent, /EnGettingStarted/);
assert.match(docsContent, /ZhGettingStarted/);
assert.match(docsContent, /"getting-started": EnGettingStarted/);
assert.match(docsContent, /getDocsContent/);
assert.match(docsContent, /getRegisteredDocsSlugs/);

assert.match(docsIndexPage, /getDocsContent\(locale\)/);
assert.match(docsIndexPage, /DocsContentShell/);
assert.match(docsSlugPage, /generateStaticParams/);
assert.match(docsSlugPage, /getRegisteredDocsSlugs\(locale\)/);
assert.match(docsSlugPage, /getDocsContent\(locale, normalizedSlug\)/);
assert.match(docsSlugPage, /getDocsTableOfContents\(locale, normalizedSlug\)/);
assert.match(docsSlugPage, /notFound\(\)/);

for (const source of [enGuide, zhGuide]) {
  assert.match(source, /^---\ntitle: /m);
  assert.match(source, /\nsection: /);
  assert.match(source, /\norder: 2/);
  assert.match(source, /127\.0\.0\.1:8787/);
  assert.match(source, /sk-local-\.\.\./);
  assert.match(source, /Cursor/);
  assert.match(source, /Claude Desktop/);
  assert.match(
    source,
    /curl http:\/\/127\.0\.0\.1:8787\/v1\/chat\/completions/,
  );
}

assert.match(enGuide, /Apple Silicon Mac/);
assert.match(enGuide, /Add Your First Provider/);
assert.match(enGuide, /Connect Cursor/);
assert.match(enGuide, /Connect Claude Desktop/);
assert.match(zhGuide, /Apple Silicon Mac/);
assert.match(zhGuide, /添加第一个供应商/);
assert.match(zhGuide, /接入 Cursor/);
assert.match(zhGuide, /接入 Claude Desktop/);
assert.match(enIndex, /\[Getting Started\]\(\/en\/docs\/getting-started\)/);
assert.match(zhIndex, /\[入门指南\]\(\/zh\/docs\/getting-started\)/);

console.log("Docs getting started contract passed");
