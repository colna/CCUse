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

assert.ok(
  existsSync(
    path.join(root, "components/download-platform-recommendation.tsx"),
  ),
  "download platform recommendation component must exist",
);
assert.ok(
  existsSync(path.join(root, "lib/download-platform.ts")),
  "download platform detection library must exist",
);

const page = read("app/[locale]/download/page.tsx");
const component = read("components/download-platform-recommendation.tsx");
const platformLib = read("lib/download-platform.ts");
const messages = {
  en: readJson("messages/en.json"),
  zh: readJson("messages/zh.json"),
};

assert.match(page, /DownloadPlatformRecommendation/);
assert.match(page, /toAssetCandidate/);
assert.match(page, /getPlatformLabels/);
assert.match(page, /formatBytes\(asset\.size\)/);

assert.match(component, /^"use client";/);
assert.match(component, /navigatorWithHints\.userAgent/);
assert.match(component, /getHighEntropyValues/);
assert.match(component, /architecture/);
assert.match(component, /detectPlatformFromUserAgent/);
assert.match(component, /findRecommendedAsset/);
assert.match(component, /platformRecommendationIds\.map/);

assert.match(platformLib, /"macos-aarch64"/);
assert.match(platformLib, /"macos-x64"/);
assert.match(platformLib, /"windows-x64"/);
assert.match(platformLib, /combined\.includes\("windows"\)/);
assert.match(platformLib, /normalizedArchitecture\.includes\("arm"\)/);
assert.match(platformLib, /normalizedArchitecture\.includes\("x86"\)/);
assert.match(platformLib, /endsWith\("_aarch64\.dmg"\)/);
assert.match(platformLib, /endsWith\("_x64\.dmg"\)/);
assert.match(platformLib, /endsWith\("_x64-setup\.exe"\)/);

for (const [locale, bundle] of Object.entries(messages)) {
  const platform = bundle.DownloadPage.platform;

  assert.ok(platform.title, `${locale} platform title missing`);
  assert.ok(platform.description, `${locale} platform description missing`);
  assert.ok(platform.detectedLabel, `${locale} detected label missing`);
  assert.ok(platform.unknownTitle, `${locale} unknown title missing`);
  assert.ok(
    platform.unknownDescription,
    `${locale} unknown description missing`,
  );
  assert.ok(platform.noAsset, `${locale} no asset label missing`);
  assert.ok(
    platform.downloadRecommended,
    `${locale} recommended download label missing`,
  );
  assert.deepEqual(Object.keys(platform.options), [
    "macosAarch64",
    "macosX64",
    "windowsX64",
  ]);
}

console.log("Download platform contract passed");
