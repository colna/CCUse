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

const rootPackage = readJson("package.json");
const desktopPackage = readJson("apps/desktop/package.json");
const websitePackage = readJson("apps/website/package.json");
const uiPackage = readJson("packages/ui/package.json");
const prettierConfig = readJson(".prettierrc.json");
const eslintConfig = read("eslint.config.mjs");

for (const dependency of [
  "@eslint/js",
  "eslint",
  "eslint-plugin-jsx-a11y",
  "eslint-plugin-react",
  "eslint-plugin-react-hooks",
  "eslint-plugin-react-refresh",
  "globals",
  "typescript-eslint",
]) {
  assert.equal(typeof rootPackage.devDependencies[dependency], "string");
  assert.equal(desktopPackage.devDependencies[dependency], undefined);
}

assert.equal(
  existsSync(path.join(root, "apps/desktop/eslint.config.mjs")),
  false,
);
assert.match(
  rootPackage.scripts.lint,
  /eslint apps\/desktop apps\/website packages\/ui/,
);
assert.match(
  rootPackage.scripts["lint:fix"],
  /eslint apps\/desktop apps\/website packages\/ui .*--fix/,
);
assert.equal(
  rootPackage.scripts["website:verify-lint"],
  "node scripts/verify-phase-1-0-w-lint.mjs",
);

for (const packageJson of [desktopPackage, websitePackage, uiPackage]) {
  assert.match(packageJson.scripts.lint, /pnpm --dir \.\.\/\.\. exec eslint/);
  assert.match(
    packageJson.scripts["lint:fix"],
    /pnpm --dir \.\.\/\.\. exec eslint/,
  );
}

assert.ok(
  rootPackage["lint-staged"][
    "{apps/desktop,apps/website,packages/ui}/**/*.{ts,tsx,js,mjs}"
  ],
);
assert.ok(
  rootPackage["lint-staged"][
    "{apps/desktop,apps/website,packages/ui}/**/*.{css,json,md}"
  ],
);
assert.match(eslintConfig, /typescript-eslint/);
assert.match(eslintConfig, /eslint-plugin-react/);
assert.match(eslintConfig, /eslint-plugin-jsx-a11y/);
assert.match(eslintConfig, /\*\*\/\.next\/\*\*/);
assert.match(eslintConfig, /\*\*\/scripts\/\*\*\/\*\.\{ts,js,mjs\}/);
assert.match(eslintConfig, /apps\/website\/app\/\*\*\/\*\.\{ts,tsx\}/);

const prettierOverrides = prettierConfig.overrides.map((override) => ({
  files: override.files.join(" "),
  tailwindConfig: override.options?.tailwindConfig,
}));
assert.ok(
  prettierOverrides.some(
    (override) =>
      override.files.includes("apps/desktop") &&
      override.tailwindConfig === "./apps/desktop/tailwind.config.ts",
  ),
);
assert.ok(
  prettierOverrides.some(
    (override) =>
      override.files.includes("apps/website") &&
      override.tailwindConfig === "./apps/website/tailwind.config.ts",
  ),
);
assert.ok(
  prettierOverrides.some(
    (override) =>
      override.files.includes("packages/ui") &&
      override.tailwindConfig === "./apps/website/tailwind.config.ts",
  ),
);

console.log("Phase 1.0.W lint/prettier contract passed");
