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
  existsSync(path.join(root, "app/[locale]/download/page.tsx")),
  "localized download route must exist",
);

const page = read("app/[locale]/download/page.tsx");
const sitemap = read("app/sitemap.ts");
const messages = {
  en: readJson("messages/en.json"),
  zh: readJson("messages/zh.json"),
};

assert.doesNotMatch(
  page,
  /use client/,
  "download page must stay a Server Component",
);
assert.match(page, /export const revalidate = 60/);
assert.match(
  page,
  /https:\/\/api\.github\.com\/repos\/colna\/CCUse\/releases\/latest/,
);
assert.match(page, /fetch\(latestReleaseApiUrl/);
assert.match(page, /next: \{ revalidate \}/);
assert.match(page, /application\/vnd\.github\+json/);
assert.match(page, /X-GitHub-Api-Version/);
assert.match(page, /normalizeRelease/);
assert.match(page, /browser_download_url/);
assert.match(page, /content_type/);
assert.match(page, /assets\.map/);
assert.match(page, /generateMetadata/);
assert.match(page, /setRequestLocale\(locale\)/);
assert.match(
  page,
  /getTranslations\(\{ locale, namespace: "DownloadPage" \}\)/,
);

assert.match(sitemap, /absoluteUrl\(`\/\$\{locale\}\/download`\)/);
assert.match(sitemap, /changeFrequency: "hourly"/);

for (const [locale, bundle] of Object.entries(messages)) {
  const download = bundle.DownloadPage;

  assert.ok(download.metadata.title, `${locale} metadata title missing`);
  assert.ok(
    download.metadata.description,
    `${locale} metadata description missing`,
  );
  assert.ok(download.hero.title, `${locale} hero title missing`);
  assert.ok(download.hero.primaryAction, `${locale} primary action missing`);
  assert.ok(download.release.latest, `${locale} release latest missing`);
  assert.ok(download.release.assetCount, `${locale} asset count missing`);
  assert.ok(download.assets.title, `${locale} assets title missing`);
  assert.ok(download.assets.downloadAsset, `${locale} download label missing`);
}

console.log("Download release contract passed");
