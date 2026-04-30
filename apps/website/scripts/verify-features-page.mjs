import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.cwd();
const featureKeys = [
  "failover",
  "multiProvider",
  "healthCheck",
  "smartStrategy",
  "monitoring",
  "crossPlatform",
];
const screenshotKeys = ["setup", "routing", "result"];
const detailKeys = ["first", "second", "third"];

function read(relativePath) {
  return readFileSync(path.join(root, relativePath), "utf8");
}

function readJson(relativePath) {
  return JSON.parse(read(relativePath));
}

assert.ok(
  existsSync(path.join(root, "app/[locale]/features/page.tsx")),
  "localized features route must exist",
);

const featuresPage = read("app/[locale]/features/page.tsx");
const homePage = read("app/[locale]/page.tsx");
const header = read("components/site-header.tsx");
const sitemap = read("app/sitemap.ts");
const messages = {
  en: readJson("messages/en.json"),
  zh: readJson("messages/zh.json"),
};

assert.doesNotMatch(
  featuresPage,
  /use client/,
  "features page must stay a Server Component",
);
assert.match(featuresPage, /generateMetadata/);
assert.match(featuresPage, /setRequestLocale\(locale\)/);
assert.match(
  featuresPage,
  /getTranslations\(\{ locale, namespace: "FeaturesPage" \}\)/,
);
assert.match(featuresPage, /aria-labelledby="features-hero-title"/);
assert.match(
  featuresPage,
  /const screenshotKeys = \["setup", "routing", "result"\] as const/,
);
assert.match(featuresPage, /id=\{key\}/);
assert.match(featuresPage, /FeatureScreenshot/);
assert.match(featuresPage, /@ccuse\/ui\/card/);
assert.match(featuresPage, /lucide-react/);

for (const key of featureKeys) {
  assert.match(featuresPage, new RegExp(`key: "${key}"`));
  assert.match(homePage, new RegExp(`/features#\\$\\{key\\}`));
}

assert.match(homePage, /featuresCta/);
assert.match(header, /\{ key: "features", href: "\/features" \}/);
assert.match(sitemap, /absoluteUrl\(`\/\$\{locale\}\/features`\)/);
assert.match(sitemap, /priority: 0\.9/);

for (const [locale, bundle] of Object.entries(messages)) {
  const features = bundle.FeaturesPage;

  assert.ok(features.metadata.title, `${locale} metadata title missing`);
  assert.ok(features.metadata.description, `${locale} metadata missing`);
  assert.ok(features.hero.title, `${locale} hero title missing`);
  assert.ok(features.overview.title, `${locale} overview missing`);
  assert.ok(features.screenshotsLabel, `${locale} screenshots label missing`);
  assert.deepEqual(Object.keys(features.features), featureKeys);

  for (const key of featureKeys) {
    const section = features.features[key];

    assert.ok(section.eyebrow, `${locale} ${key} eyebrow missing`);
    assert.ok(section.title, `${locale} ${key} title missing`);
    assert.ok(section.summary, `${locale} ${key} summary missing`);
    assert.ok(section.description, `${locale} ${key} description missing`);
    assert.deepEqual(Object.keys(section.details), detailKeys);
    assert.deepEqual(Object.keys(section.shots), screenshotKeys);

    for (const detailKey of detailKeys) {
      assert.ok(
        section.details[detailKey].label,
        `${locale} ${key} detail ${detailKey} label missing`,
      );
      assert.ok(
        section.details[detailKey].value,
        `${locale} ${key} detail ${detailKey} value missing`,
      );
    }

    for (const screenshotKey of screenshotKeys) {
      const screenshot = section.shots[screenshotKey];

      assert.ok(
        screenshot.status,
        `${locale} ${key} screenshot ${screenshotKey} status missing`,
      );
      assert.ok(
        screenshot.title,
        `${locale} ${key} screenshot ${screenshotKey} title missing`,
      );
      assert.ok(
        screenshot.caption,
        `${locale} ${key} screenshot ${screenshotKey} caption missing`,
      );
      assert.ok(
        screenshot.footer,
        `${locale} ${key} screenshot ${screenshotKey} footer missing`,
      );
      assert.deepEqual(Object.keys(screenshot.rows), detailKeys);
    }
  }
}

console.log("Features page contract passed");
