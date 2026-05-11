import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.cwd();

function readJson(relativePath) {
  return JSON.parse(readFileSync(path.join(root, relativePath), "utf8"));
}

function readText(relativePath) {
  return readFileSync(path.join(root, relativePath), "utf8");
}

function assertPath(relativePath) {
  assert.ok(
    existsSync(path.join(root, relativePath)),
    `${relativePath} must exist`,
  );
}

// 1) turbo.json 存在 & schema 关键字段齐
assertPath("turbo.json");
const turbo = readJson("turbo.json");
assert.equal(
  turbo.$schema,
  "https://turbo.build/schema.json",
  "turbo.json must declare $schema",
);
for (const taskName of [
  "build",
  "typecheck",
  "test",
  "lint",
  "lint:fix",
  "dev",
  "tauri:dev",
  "tauri:build",
]) {
  assert.ok(
    turbo.tasks && turbo.tasks[taskName],
    `turbo.json tasks.${taskName} must exist`,
  );
}
assert.equal(
  turbo.tasks.dev.cache,
  false,
  "turbo.json tasks.dev.cache must be false",
);
assert.equal(
  turbo.tasks.dev.persistent,
  true,
  "turbo.json tasks.dev.persistent must be true",
);
assert.equal(
  turbo.tasks["tauri:dev"].cache,
  false,
  "turbo.json tasks['tauri:dev'].cache must be false",
);
assert.equal(
  turbo.tasks["tauri:build"].cache,
  false,
  "turbo.json tasks['tauri:build'].cache must be false",
);
assert.deepEqual(
  turbo.tasks.build.dependsOn,
  ["^build"],
  "turbo.json tasks.build.dependsOn must be ['^build']",
);

// 2) root package.json — turbo devDep + scripts 切换
const rootPackage = readJson("package.json");
assert.ok(
  rootPackage.devDependencies && rootPackage.devDependencies.turbo,
  "root package.json must declare devDependencies.turbo",
);

const requiredScripts = [
  "dev:desktop",
  "dev:website",
  "build",
  "build:desktop",
  "build:website",
  "test",
  "test:desktop",
  "test:website",
  "typecheck",
  "lint",
  "lint:fix",
  "verify-turbo",
];
for (const name of requiredScripts) {
  assert.ok(
    rootPackage.scripts && rootPackage.scripts[name],
    `root scripts.${name} must exist`,
  );
}

const turboBackedScripts = [
  "dev:desktop",
  "dev:website",
  "build",
  "build:desktop",
  "build:website",
  "test",
  "test:desktop",
  "test:website",
  "typecheck",
  "lint",
  "lint:fix",
];
for (const name of turboBackedScripts) {
  assert.match(
    rootPackage.scripts[name],
    /^turbo run /,
    `root scripts.${name} must invoke turbo run`,
  );
}

const removedScripts = [
  "desktop:dev",
  "desktop:build",
  "desktop:test",
  "desktop:typecheck",
  "website:dev",
  "website:build",
  "website:test",
  "website:typecheck",
];
for (const name of removedScripts) {
  assert.ok(
    !(rootPackage.scripts && rootPackage.scripts[name]),
    `root scripts.${name} must be removed`,
  );
}

// 3) apps/desktop — tauri:dev / tauri:build 存在；裸 tauri 透传保留
const desktopPackage = readJson("apps/desktop/package.json");
assert.equal(
  desktopPackage.scripts["tauri:dev"],
  "tauri dev",
  "apps/desktop scripts.tauri:dev must be 'tauri dev'",
);
assert.equal(
  desktopPackage.scripts["tauri:build"],
  "tauri build",
  "apps/desktop scripts.tauri:build must be 'tauri build'",
);
assert.ok(
  desktopPackage.scripts.tauri,
  "apps/desktop scripts.tauri (passthrough) must remain for CI compatibility",
);

// 4) .gitignore 含 .turbo
const gitignore = readText(".gitignore");
assert.match(
  gitignore,
  /^\.turbo\/?$/m,
  ".gitignore must include `.turbo/` to ignore turbo local cache",
);

// 5) pnpm-workspace.yaml 未被破坏
const workspace = readText("pnpm-workspace.yaml");
assert.match(workspace, /-\s+"apps\/\*"/, "workspace must include apps/*");
assert.match(
  workspace,
  /-\s+"packages\/\*"/,
  "workspace must include packages/*",
);

console.log("T1.0.W.TURBO turbo contract passed");
