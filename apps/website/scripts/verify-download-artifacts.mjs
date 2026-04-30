import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";

const root = process.cwd();
const targetKeys = ["macosAarch64", "macosX64", "windowsX64"];

function read(relativePath) {
  return readFileSync(path.join(root, relativePath), "utf8");
}

function readJson(relativePath) {
  return JSON.parse(read(relativePath));
}

const page = read("app/[locale]/download/page.tsx");
const platformLib = read("lib/download-platform.ts");
const messages = {
  en: readJson("messages/en.json"),
  zh: readJson("messages/zh.json"),
};

assert.match(page, /const downloadTargets = \[/);
assert.match(page, /id="download-packages"/);
assert.match(page, /DownloadPackagesSection/);
assert.match(page, /DownloadPackageCard/);
assert.match(page, /findTargetAsset/);
assert.match(page, /matchesAssetName\(asset\.name, platformId\)/);
assert.match(page, /href="#download-packages"/);

for (const platformId of ["macos-aarch64", "macos-x64", "windows-x64"]) {
  assert.match(page, new RegExp(`platformId: "${platformId}"`));
}

assert.match(platformLib, /normalizedName\.endsWith\("_aarch64\.dmg"\)/);
assert.match(platformLib, /normalizedName\.endsWith\("_x64\.dmg"\)/);
assert.match(platformLib, /!normalizedName\.endsWith\("_x64-setup\.exe"\)/);
assert.match(platformLib, /normalizedName\.endsWith\("_x64-setup\.exe"\)/);

for (const [locale, bundle] of Object.entries(messages)) {
  const packages = bundle.DownloadPage.packages;

  assert.ok(packages.eyebrow, `${locale} packages eyebrow missing`);
  assert.ok(packages.title, `${locale} packages title missing`);
  assert.ok(packages.description, `${locale} packages description missing`);
  assert.ok(packages.filenamePattern, `${locale} filename label missing`);
  assert.ok(packages.status, `${locale} status label missing`);
  assert.ok(packages.size, `${locale} size label missing`);
  assert.ok(packages.missing, `${locale} missing label missing`);
  assert.ok(
    packages.missingDescription,
    `${locale} missing description missing`,
  );
  assert.ok(packages.download, `${locale} download label missing`);
  assert.deepEqual(Object.keys(packages.targets), targetKeys);
  assert.equal(packages.targets.macosAarch64.pattern, "*_aarch64.dmg");
  assert.equal(packages.targets.macosX64.pattern, "*_x64.dmg");
  assert.equal(packages.targets.windowsX64.pattern, "*_x64-setup.exe");
}

console.log("Download artifacts contract passed");
