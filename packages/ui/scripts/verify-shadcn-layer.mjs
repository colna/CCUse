import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";

const root = process.cwd();
const repoRoot = path.join(root, "../..");

function read(relativePath) {
  return readFileSync(path.join(root, relativePath), "utf8");
}

function readRepo(relativePath) {
  return readFileSync(path.join(repoRoot, relativePath), "utf8");
}

const packageJson = JSON.parse(read("package.json"));
const index = read("index.ts");
const button = read("src/components/button.tsx");
const card = read("src/components/card.tsx");
const dialog = read("src/components/dialog.tsx");
const desktopButton = readRepo("apps/desktop/src/components/ui/button.tsx");
const websitePage = readRepo("apps/website/app/[locale]/page.tsx");
const websiteHeader = readRepo("apps/website/components/site-header.tsx");
const websiteNextConfig = readRepo("apps/website/next.config.mjs");

for (const subpath of ["./button", "./card", "./dialog"]) {
  assert.equal(typeof packageJson.exports[subpath].default, "string");
  assert.equal(typeof packageJson.exports[subpath].types, "string");
}

assert.match(index, /export \* from "\.\/src\/components\/button"/);
assert.match(index, /export \* from "\.\/src\/components\/card"/);
assert.match(index, /export \* from "\.\/src\/components\/dialog"/);
assert.match(button, /buttonVariants/);
assert.match(button, /@radix-ui\/react-slot/);
assert.match(card, /CardContent/);
assert.match(dialog, /@radix-ui\/react-dialog/);
assert.match(dialog, /lucide-react/);
assert.match(desktopButton, /@ccuse\/ui\/button/);
assert.match(websitePage, /@ccuse\/ui\/button/);
assert.match(websitePage, /@ccuse\/ui\/card/);
assert.match(websiteHeader, /\/icon\.png/);
assert.match(websiteNextConfig, /transpilePackages:\s*\["@ccuse\/ui"\]/);

console.log("Shared shadcn UI contract passed");
