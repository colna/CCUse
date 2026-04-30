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
const docsLib = read("lib/docs.ts");
const docsLayout = read("app/[locale]/docs/layout.tsx");
const docsPage = read("app/[locale]/docs/page.tsx");
const enIndex = read("content/docs/en/index.mdx");
const zhIndex = read("content/docs/zh/index.mdx");
const enMessages = readJson("messages/en.json");
const zhMessages = readJson("messages/zh.json");

for (const requiredPath of [
  "app/[locale]/docs/layout.tsx",
  "app/[locale]/docs/page.tsx",
  "content/docs/en/index.mdx",
  "content/docs/zh/index.mdx",
  "lib/docs.ts",
]) {
  assertPath(requiredPath);
}

assert.ok(
  packageJson.dependencies?.["gray-matter"],
  "website must depend on gray-matter for frontmatter parsing",
);
assert.match(docsLib, /node:fs/);
assert.match(docsLib, /gray-matter/);
assert.match(docsLib, /content\/docs/);
assert.match(docsLib, /getDocsNavigation/);
assert.match(docsLib, /collectMdxFiles/);
assert.match(docsLib, /readDocNavigationItem/);
assert.match(docsLib, /replace\(\/\\\/index\$\/, ""\)/);

assert.match(docsLayout, /getDocsNavigation\(locale\)/);
assert.match(docsLayout, /<details[\s\S]*open/);
assert.match(docsLayout, /<summary/);
assert.match(docsLayout, /sidebarLabel/);
assert.match(docsLayout, /sidebarTitle/);
assert.match(docsLayout, /max-w-7xl/);
assert.match(docsLayout, /lg:grid-cols-\[17rem_minmax\(0,1fr\)\]/);
assert.match(docsLayout, /lg:sticky lg:top-24/);

assert.match(docsPage, /DocsContentShell/);
assert.match(docsPage, /getDocsContent\(locale\)/);
assert.match(docsPage, /<Content \/>/);

for (const source of [enIndex, zhIndex]) {
  assert.match(source, /^---\ntitle: /m);
  assert.match(source, /\nsection: /);
  assert.match(source, /\norder: 1/);
  assert.match(source, /```ts\nconst baseUrl/);
}

assert.deepEqual(Object.keys(enMessages.Docs), [
  "searchEmpty",
  "searchError",
  "searchLabel",
  "searchLoading",
  "searchPlaceholder",
  "sidebarLabel",
  "sidebarTitle",
  "tocLabel",
]);
assert.deepEqual(Object.keys(zhMessages.Docs), [
  "searchEmpty",
  "searchError",
  "searchLabel",
  "searchLoading",
  "searchPlaceholder",
  "sidebarLabel",
  "sidebarTitle",
  "tocLabel",
]);

console.log("Docs sidebar contract passed");
