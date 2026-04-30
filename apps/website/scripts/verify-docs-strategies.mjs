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
const docsContent = read("lib/docs-content.ts");
const sitemap = read("app/sitemap.ts");
const enIndex = read("content/docs/en/index.mdx");
const zhIndex = read("content/docs/zh/index.mdx");
const enGuide = read("content/docs/en/switching-strategies.mdx");
const zhGuide = read("content/docs/zh/switching-strategies.mdx");
const switchStrategy = readRepo(
  "apps/desktop/src-tauri/src/switch/strategy.rs",
);

for (const requiredPath of [
  "content/docs/en/switching-strategies.mdx",
  "content/docs/zh/switching-strategies.mdx",
]) {
  assertPath(requiredPath);
}

assert.match(packageJson.scripts.test, /verify-docs-strategies\.mjs/);
assert.match(docsContent, /EnSwitchingStrategies/);
assert.match(docsContent, /ZhSwitchingStrategies/);
assert.match(docsContent, /"switching-strategies": EnSwitchingStrategies/);
assert.match(docsContent, /"switching-strategies": ZhSwitchingStrategies/);
assert.match(sitemap, /getRegisteredDocsSlugs/);

for (const strategy of [
  "Priority",
  "Fastest",
  "Cost",
  "LoadBalance",
  "Smart",
]) {
  assert.match(switchStrategy, new RegExp(`\\b${strategy}\\b`));
}

for (const source of [enGuide, zhGuide]) {
  assert.match(source, /^---\ntitle: /m);
  assert.match(source, /\nsection: /);
  assert.match(source, /\norder: 3/);
  assert.match(source, /`priority`/);
  assert.match(source, /`fastest`/);
  assert.match(source, /`cost`/);
  assert.match(source, /`load_balance`/);
  assert.match(source, /`smart`/);
  assert.match(source, /40/);
  assert.match(source, /30/);
  assert.match(source, /20/);
  assert.match(source, /10/);
  assert.match(source, /round-robin/i);
}

assert.match(enGuide, /Quick Choice/);
assert.match(enGuide, /Priority/);
assert.match(enGuide, /Fastest/);
assert.match(enGuide, /Cost First/);
assert.match(enGuide, /Load Balance/);
assert.match(enGuide, /Smart/);
assert.match(enGuide, /Failure Behavior/);

assert.match(zhGuide, /快速选择/);
assert.match(zhGuide, /优先级/);
assert.match(zhGuide, /最快响应/);
assert.match(zhGuide, /成本优先/);
assert.match(zhGuide, /负载均衡/);
assert.match(zhGuide, /智能策略/);
assert.match(zhGuide, /失败处理/);

assert.match(
  enIndex,
  /\[Switching Strategies\]\(\/en\/docs\/switching-strategies\)/,
);
assert.match(zhIndex, /\[切换策略\]\(\/zh\/docs\/switching-strategies\)/);

console.log("Docs switching strategies contract passed");
