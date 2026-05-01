import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";

const root = process.cwd();

function read(relativePath) {
  return readFileSync(path.join(root, relativePath), "utf8");
}

function readRepo(relativePath) {
  return readFileSync(path.join(root, "../..", relativePath), "utf8");
}

function readJson(relativePath) {
  return JSON.parse(read(relativePath));
}

const page = read("app/[locale]/download/page.tsx");
const releaseLib = read("lib/github-release.ts");
const workflow = readRepo(".github/workflows/release.yml");
const workflowContract = readRepo(
  "apps/desktop/src-tauri/tests/release_workflow.rs",
);
const messages = {
  en: readJson("messages/en.json"),
  zh: readJson("messages/zh.json"),
};

assert.match(page, /asset\.sha256 \?\? t\("packages\.sha256Missing"\)/);
assert.match(page, /asset\.sha256 \?\? t\("assets\.sha256Missing"\)/);
assert.match(page, /type ReleaseAsset/);

assert.match(releaseLib, /checksumSuffixes/);
assert.match(releaseLib, /\.sha256/);
assert.match(releaseLib, /Promise\.all\(/);
assert.match(releaseLib, /parseChecksumText/);
assert.match(releaseLib, /hashMatch\[0\]\.toLowerCase\(\)/);
assert.match(releaseLib, /checksumAssetName/);

assert.match(workflow, /Upload SHA-256 checksum/);
assert.match(workflow, /missing_macos_aarch64_checksum/);
assert.match(workflow, /\$MACOS_AARCH64_ASSET\.sha256/);
assert.match(workflow, /crypto\.createHash\('sha256'\)/);
assert.match(
  workflow,
  /gh release upload "\$TAG" "\$CHECKSUM_DIR\/\$ASSET_NAME\.sha256" --clobber/,
);

assert.match(workflowContract, /Upload SHA-256 checksum/);
assert.match(workflowContract, /checksum_missing/);

for (const [locale, bundle] of Object.entries(messages)) {
  assert.equal(
    typeof bundle.DownloadPage.packages.sha256,
    "string",
    `${locale} package checksum label missing`,
  );
  assert.equal(
    typeof bundle.DownloadPage.packages.sha256Missing,
    "string",
    `${locale} package checksum missing label missing`,
  );
  assert.equal(
    typeof bundle.DownloadPage.assets.sha256,
    "string",
    `${locale} raw asset checksum label missing`,
  );
  assert.equal(
    typeof bundle.DownloadPage.assets.sha256Missing,
    "string",
    `${locale} raw asset checksum missing label missing`,
  );
}

console.log("Download checksums contract passed");
