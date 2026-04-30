import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";

const root = process.cwd();

function read(relativePath) {
  return readFileSync(path.join(root, relativePath), "utf8");
}

const packageJson = JSON.parse(read("package.json"));
const tailwindConfig = read("tailwind.config.ts");
const postcssConfig = read("postcss.config.mjs");
const layout = read("app/[locale]/layout.tsx");
const globals = read("app/globals.css");
const uiPackage = JSON.parse(
  readFileSync(path.join(root, "../../packages/ui/package.json"), "utf8"),
);
const uiPreset = readFileSync(
  path.join(root, "../../packages/ui/tailwind-preset.js"),
  "utf8",
);

assert.equal(uiPackage.name, "@ccuse/ui");
assert.equal(
  uiPackage.exports["./tailwind-preset"].types,
  "./tailwind-preset.d.ts",
);
assert.equal(
  uiPackage.exports["./tailwind-preset"].default,
  "./tailwind-preset.js",
);
assert.match(tailwindConfig, /@ccuse\/ui\/tailwind-preset/);
assert.match(tailwindConfig, /\.\.\/\.\.\/packages\/ui/);
assert.match(postcssConfig, /tailwindcss/);
assert.match(postcssConfig, /autoprefixer/);
assert.match(layout, /import "\.\.\/globals\.css"/);
assert.match(globals, /@tailwind base/);
assert.match(globals, /--primary:\s*212 100% 45%/);
assert.match(uiPreset, /apple-headline/);
assert.match(uiPreset, /tailwindcss-animate/);
assert.equal(packageJson.devDependencies?.["@ccuse/ui"], "workspace:*");
assert.match(packageJson.devDependencies?.tailwindcss ?? "", /^\^?3\./);

console.log("Tailwind website contract passed");
