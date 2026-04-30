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
const layout = read("app/layout.tsx");
const page = read("app/page.tsx");
const config = read("next.config.mjs");

assert.equal(packageJson.name, "@ccuse/website");
assert.equal(packageJson.private, true);
assert.equal(packageJson.scripts.build, "next build");
assert.equal(packageJson.scripts.typecheck, "tsc --noEmit");
assert.match(packageJson.dependencies?.next ?? "", /^\^?14\./);
assert.match(packageJson.dependencies?.react ?? "", /^\^?18\./);
assert.match(packageJson.dependencies?.["react-dom"] ?? "", /^\^?18\./);

for (const requiredPath of [
  "app/layout.tsx",
  "app/page.tsx",
  "next.config.mjs",
  "next-env.d.ts",
  "tsconfig.json",
]) {
  assertPath(requiredPath);
}

assert.match(
  layout,
  /export const metadata/,
  "root layout must export Metadata API config",
);
assert.match(
  layout,
  /export const viewport/,
  "root layout must export viewport config",
);
assert.doesNotMatch(
  layout,
  /use client/,
  "root layout must stay a Server Component",
);
assert.doesNotMatch(
  page,
  /use client/,
  "home page must stay a Server Component",
);
assert.match(
  page,
  /href="\/download"/,
  "home page must expose the download CTA",
);
assert.match(
  config,
  /reactStrictMode:\s*true/,
  "Next config must keep React strict mode",
);

console.log("Next.js website contract passed");
