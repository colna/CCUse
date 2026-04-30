import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.cwd();
const repoRoot = path.join(root, "../..");
const architectureColumns = ["client", "proxy", "providers"];
const architectureFlows = ["request", "selection", "recovery"];

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

const page = read("app/[locale]/page.tsx");
const architecture = read("lib/architecture.ts");
const productDoc = readRepo("docs/产品技术文档.md");
const mobileFallback = read("public/architecture-mobile.svg");
const messages = {
  en: readJson("messages/en.json"),
  zh: readJson("messages/zh.json"),
};

for (const requiredPath of [
  "lib/architecture.ts",
  "public/architecture-mobile.svg",
]) {
  assertPath(requiredPath);
}

assert.match(productDoc, /subgraph Client\["用户客户端"\]/);
assert.match(productDoc, /本地 API 代理服务<br\/>\(Rust HTTP Server\)/);
assert.match(productDoc, /Claude Provider/);
assert.match(architecture, /export const architectureMermaid/);
assert.match(architecture, /subgraph Client\["用户客户端"\]/);
assert.match(architecture, /本地 API 代理服务<br\/>\(Rust HTTP Server\)/);
assert.match(architecture, /H -->\|失败\| I/);
assert.match(architecture, /J -\.定时检查\.-> K/);

assert.match(page, /architectureMermaid/);
assert.match(page, /id="architecture"/);
assert.match(page, /id="architecture-title"/);
assert.match(page, /aria-labelledby="architecture-title"/);
assert.match(page, /hidden rounded-lg[\s\S]*lg:block/);
assert.match(page, /lg:hidden/);
assert.match(page, /src="\/architecture-mobile\.svg"/);
assert.match(page, /<pre className="sr-only">\{architectureMermaid\}<\/pre>/);
assert.match(page, /role="img"/);
assert.match(page, /from "next\/image"/);

assert.match(mobileFallback, /<svg/);
assert.match(mobileFallback, /CLIENTS/);
assert.match(mobileFallback, /CCUse LOCAL PROXY/);
assert.match(mobileFallback, /PROVIDERS/);
assert.doesNotMatch(mobileFallback, /href=/);

for (const [locale, bundle] of Object.entries(messages)) {
  const architectureMessages = bundle.HomePage.architecture;

  assert.ok(architectureMessages.eyebrow, `${locale} eyebrow missing`);
  assert.ok(architectureMessages.title, `${locale} title missing`);
  assert.ok(architectureMessages.description, `${locale} description missing`);
  assert.ok(
    architectureMessages.diagramLabel,
    `${locale} diagram label missing`,
  );
  assert.ok(architectureMessages.mobileAlt, `${locale} mobile alt missing`);
  assert.deepEqual(
    Object.keys(architectureMessages.columns),
    architectureColumns,
  );
  assert.deepEqual(Object.keys(architectureMessages.flows), architectureFlows);
  assert.deepEqual(
    Object.keys(architectureMessages.flowDescriptions),
    architectureFlows,
  );

  for (const column of architectureColumns) {
    assert.ok(
      architectureMessages.columns[column].title,
      `${locale} ${column} title missing`,
    );
    assert.ok(
      Object.keys(architectureMessages.columns[column].nodes).length >= 3,
      `${locale} ${column} nodes incomplete`,
    );
  }
}

console.log("Home architecture contract passed");
