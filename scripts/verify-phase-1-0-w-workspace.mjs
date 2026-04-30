import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.cwd();

function readJson(relativePath) {
  return JSON.parse(readFileSync(path.join(root, relativePath), "utf8"));
}

function assertPath(relativePath) {
  assert.ok(
    existsSync(path.join(root, relativePath)),
    `${relativePath} must exist`,
  );
}

const workspace = readFileSync(path.join(root, "pnpm-workspace.yaml"), "utf8");
const rootPackage = readJson("package.json");
const desktopPackage = readJson("apps/desktop/package.json");

assert.match(workspace, /-\s+"apps\/\*"/, "workspace must include apps/*");
assert.match(
  workspace,
  /-\s+"packages\/\*"/,
  "workspace must include packages/*",
);
assert.equal(rootPackage.private, true, "root package must stay private");
assert.match(
  rootPackage.packageManager,
  /^pnpm@/,
  "root package manager must be pnpm",
);
assert.equal(
  desktopPackage.name,
  "@ccuse/desktop",
  "desktop package must remain namespaced",
);

for (const requiredPath of [
  "apps/desktop/package.json",
  "apps/desktop/src-tauri/tauri.conf.json",
  "apps/website",
  "packages/ui",
]) {
  assertPath(requiredPath);
}

console.log("Phase 1.0.W workspace contract passed");
