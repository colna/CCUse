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
const nextConfig = read("next.config.mjs");
const mdxComponents = read("mdx-components.tsx");
const docsPage = read("content/docs/en/index.mdx");

for (const requiredPath of [
  "app/[locale]/docs/page.tsx",
  "content/docs/en/index.mdx",
  "content/docs/zh/index.mdx",
  "mdx-components.tsx",
]) {
  assertPath(requiredPath);
}

for (const dependency of [
  "@mdx-js/loader",
  "@mdx-js/react",
  "@next/mdx",
  "rehype-autolink-headings",
  "rehype-pretty-code",
  "rehype-slug",
  "remark-frontmatter",
  "remark-gfm",
  "shiki",
]) {
  assert.ok(
    packageJson.dependencies?.[dependency],
    `website must depend on ${dependency}`,
  );
}

assert.ok(
  packageJson.devDependencies?.["@types/mdx"],
  "website must declare @types/mdx for typed MDX components",
);
assert.match(nextConfig, /@next\/mdx/);
assert.match(nextConfig, /createMDX/);
assert.match(
  nextConfig,
  /pageExtensions: \["js", "jsx", "md", "mdx", "ts", "tsx"\]/,
);
assert.match(nextConfig, /remarkPlugins: \[remarkGfm, remarkFrontmatter\]/);
assert.match(nextConfig, /rehypePrettyCode/);
assert.match(
  nextConfig,
  /theme:\s*\{\s*dark: "github-dark",\s*light: "github-light"/s,
);
assert.match(nextConfig, /withNextIntl\(withMDX\(nextConfig\)\)/);

assert.match(mdxComponents, /useMDXComponents/);
assert.match(mdxComponents, /MDXComponents/);
assert.match(mdxComponents, /font-display text-4xl/);
assert.match(mdxComponents, /overflow-x-auto rounded-lg/);
assert.match(mdxComponents, /focus-visible:ring-2/);

assert.match(docsPage, /^---\ntitle: CCUse Docs/m);
assert.match(docsPage, /# CCUse Docs/);
assert.match(docsPage, /```ts\nconst baseUrl/);
assert.match(docsPage, /GitHub-flavored Markdown/);
assert.match(docsPage, /Shiki-powered code highlighting/);

console.log("MDX pipeline contract passed");
