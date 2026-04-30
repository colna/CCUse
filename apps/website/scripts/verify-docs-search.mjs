import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.cwd();
const repoRoot = path.join(root, "../..");

function read(relativePath) {
  return readFileSync(path.join(root, relativePath), "utf8");
}

function readRepo(relativePath) {
  return readFileSync(path.join(repoRoot, relativePath), "utf8");
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
const eslintConfig = readRepo("eslint.config.mjs");
const gitignore = readRepo(".gitignore");
const buildScript = read("scripts/build-pagefind-index.mjs");
const docsContentShell = read("components/docs-content-shell.tsx");
const docsLayout = read("app/[locale]/docs/layout.tsx");
const docsPage = read("app/[locale]/docs/page.tsx");
const docsSearch = read("components/docs-search.tsx");
const enMessages = readJson("messages/en.json");
const zhMessages = readJson("messages/zh.json");

for (const requiredPath of [
  "components/docs-search.tsx",
  "scripts/build-pagefind-index.mjs",
]) {
  assertPath(requiredPath);
}

assert.equal(packageJson.devDependencies?.pagefind, "^1.5.2");
assert.equal(
  packageJson.scripts.postbuild,
  "node scripts/build-pagefind-index.mjs",
);
assert.equal(
  packageJson.scripts.pagefind,
  "node scripts/build-pagefind-index.mjs",
);
assert.match(packageJson.scripts.test, /verify-docs-search\.mjs/);

assert.match(buildScript, /pagefind/);
assert.match(buildScript, /\.next\/server\/app/);
assert.match(buildScript, /public\/_pagefind/);
assert.match(buildScript, /--root-selector/);
assert.match(buildScript, /\[data-pagefind-body\]/);
assert.match(buildScript, /--glob/);
assert.match(buildScript, /\*\*\/\*\.html/);

assert.match(gitignore, /apps\/website\/public\/_pagefind\//);
assert.match(eslintConfig, /apps\/website\/public\/_pagefind\/\*\*/);
assert.match(docsLayout, /DocsSearch/);
assert.match(docsLayout, /searchPlaceholder/);
assert.match(docsLayout, /searchLabels/);
assert.match(docsPage, /DocsContentShell/);
assert.match(docsContentShell, /data-pagefind-body/);

assert.match(docsSearch, /"use client"/);
assert.match(docsSearch, /\/_pagefind\/pagefind\.js/);
assert.match(docsSearch, /new URL\(url, window\.location\.origin\)/);
assert.match(docsSearch, /pagefind\.search\(trimmedQuery\)/);
assert.match(docsSearch, /result\.url\.startsWith\(`\/\$\{locale\}\/`\)/);
assert.match(docsSearch, /data-pagefind-ignore/);
assert.match(docsSearch, /type="search"/);
assert.match(docsSearch, /aria-label=\{labels\.label\}/);
assert.doesNotMatch(docsSearch, /dangerouslySetInnerHTML/);

for (const messages of [enMessages, zhMessages]) {
  assert.ok(messages.Docs.searchEmpty);
  assert.ok(messages.Docs.searchError);
  assert.ok(messages.Docs.searchLabel);
  assert.ok(messages.Docs.searchLoading);
  assert.ok(messages.Docs.searchPlaceholder);
}

console.log("Docs search contract passed");
