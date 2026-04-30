import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
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

function read(relativePath) {
  return readFileSync(path.join(root, relativePath), "utf8");
}

function readJson(relativePath) {
  return JSON.parse(read(relativePath));
}

const page = read("app/[locale]/page.tsx");
const messages = {
  en: readJson("messages/en.json"),
  zh: readJson("messages/zh.json"),
};

assert.doesNotMatch(
  page,
  /use client/,
  "features section must stay in the Server Component page",
);
assert.match(page, /const featureItems = \[/);
assert.match(page, /aria-labelledby="features-title"/);
assert.match(page, /id="features"/);
assert.match(page, /id="features-title"/);
assert.match(page, /featuresEyebrow/);
assert.match(page, /featuresTitle/);
assert.match(page, /featuresDescription/);
assert.match(page, /sm:grid-cols-2 lg:grid-cols-3/);
assert.match(page, /@ccuse\/ui\/card/);

for (const icon of [
  "Shuffle",
  "Network",
  "HeartPulse",
  "BrainCircuit",
  "BarChart3",
  "Laptop",
]) {
  assert.match(page, new RegExp(`\\b${icon}\\b`));
}

for (const key of featureKeys) {
  assert.match(page, new RegExp(`key: "${key}"`));
  assert.match(page, new RegExp(`features\\.\\$\\{key\\}\\.title`));
  assert.match(page, new RegExp(`features\\.\\$\\{key\\}\\.description`));
}

for (const [locale, bundle] of Object.entries(messages)) {
  const home = bundle.HomePage;

  assert.ok(home.featuresEyebrow, `${locale} features eyebrow missing`);
  assert.ok(home.featuresTitle, `${locale} features title missing`);
  assert.ok(home.featuresDescription, `${locale} features description missing`);
  assert.deepEqual(Object.keys(home.features), featureKeys);

  for (const key of featureKeys) {
    assert.ok(home.features[key].title, `${locale} ${key} title missing`);
    assert.ok(
      home.features[key].description,
      `${locale} ${key} description missing`,
    );
  }
}

console.log("Home features contract passed");
