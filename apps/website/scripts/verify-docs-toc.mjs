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
const docsPage = read("app/[locale]/docs/page.tsx");
const mdxComponents = read("mdx-components.tsx");
const tocComponent = read("components/docs-table-of-contents.tsx");
const enIndex = read("content/docs/en/index.mdx");
const zhIndex = read("content/docs/zh/index.mdx");
const enMessages = readJson("messages/en.json");
const zhMessages = readJson("messages/zh.json");

for (const requiredPath of [
  "components/docs-table-of-contents.tsx",
  "content/docs/en/index.mdx",
  "content/docs/zh/index.mdx",
  "lib/docs.ts",
]) {
  assertPath(requiredPath);
}

for (const dependency of [
  "github-slugger",
  "mdast-util-to-string",
  "remark-parse",
  "unified",
]) {
  assert.ok(
    packageJson.dependencies?.[dependency],
    `website must depend on ${dependency} for docs TOC extraction`,
  );
}

assert.match(docsLib, /GithubSlugger/);
assert.match(docsLib, /remarkParse/);
assert.match(docsLib, /toString/);
assert.match(docsLib, /getDocsTableOfContents/);
assert.match(docsLib, /child\.depth !== 2 && child\.depth !== 3/);
assert.match(docsLib, /slugger\.slug\(title\)/);
assert.match(docsLib, /resolveDocFilePath/);

assert.match(docsPage, /DocsTableOfContents/);
assert.match(docsPage, /getDocsTableOfContents\(locale\)/);
assert.match(docsPage, /tocLabel/);
assert.match(docsPage, /xl:grid-cols-\[minmax\(0,1fr\)_14rem\]/);

assert.match(tocComponent, /"use client"/);
assert.match(tocComponent, /IntersectionObserver/);
assert.match(tocComponent, /window\.addEventListener\("scroll"/);
assert.match(tocComponent, /aria-current/);
assert.match(tocComponent, /href=\{`#\$\{item\.id\}`\}/);
assert.match(tocComponent, /xl:sticky xl:top-24/);

assert.match(mdxComponents, /scroll-mt-28/);
assert.match(enIndex, /## Local Proxy Quick Start/);
assert.match(enIndex, /### Client Endpoint/);
assert.match(zhIndex, /## 本地代理快速开始/);
assert.match(zhIndex, /### 客户端端点/);
assert.equal(enMessages.Docs.tocLabel, "On this page");
assert.equal(zhMessages.Docs.tocLabel, "本页目录");

console.log("Docs table of contents contract passed");
