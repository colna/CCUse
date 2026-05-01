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

const previewPath = "app/[locale]/download/preview/page.tsx";
const previewPage = read(previewPath);
const stablePage = read("app/[locale]/download/page.tsx");
const releaseLib = read("lib/github-release.ts");
const sitemap = read("app/sitemap.ts");
const messages = {
  en: readJson("messages/en.json"),
  zh: readJson("messages/zh.json"),
};

assert.ok(existsSync(path.join(root, previewPath)), "preview page must exist");
assert.match(previewPage, /getLatestPreviewRelease/);
assert.match(previewPage, /DownloadPreviewPage/);
assert.match(previewPage, /\/download\/preview/);
assert.match(previewPage, /export const revalidate = 60/);
assert.match(previewPage, /t\("release\.prerelease"\)/);

assert.match(stablePage, /href={`\/\$\{locale\}\/download\/preview`}/);
assert.match(stablePage, /t\("hero\.previewAction"\)/);

assert.match(releaseLib, /releases\?per_page=20/);
assert.match(releaseLib, /readBoolean\(item, "prerelease"\)/);
assert.match(releaseLib, /\^v\?0\\\./);
assert.match(releaseLib, /getLatestPreviewRelease/);

assert.match(sitemap, /absoluteUrl\(`\/\$\{locale\}\/download\/preview`\)/);
assert.match(sitemap, /priority: 0\.5/);

for (const [locale, bundle] of Object.entries(messages)) {
  assert.ok(bundle.DownloadPage.hero.previewAction, `${locale} preview CTA`);
  assert.ok(bundle.DownloadPreviewPage, `${locale} preview namespace`);
  assert.ok(
    bundle.DownloadPreviewPage.risk.description,
    `${locale} preview risk copy`,
  );
  assert.ok(
    bundle.DownloadPreviewPage.release.prerelease,
    `${locale} preview release badge`,
  );
  assert.ok(
    bundle.DownloadPreviewPage.assets.sha256Missing,
    `${locale} preview checksum copy`,
  );
}

console.log("Download preview contract passed");
