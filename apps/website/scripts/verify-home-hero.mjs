import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";

const root = process.cwd();

function read(relativePath) {
  return readFileSync(path.join(root, relativePath), "utf8");
}

function readJson(relativePath) {
  return JSON.parse(read(relativePath));
}

const page = read("app/[locale]/page.tsx");
const packageJson = readJson("package.json");
const messages = {
  en: readJson("messages/en.json"),
  zh: readJson("messages/zh.json"),
};

assert.match(
  packageJson.dependencies?.["lucide-react"] ?? "",
  /^\^?0\.453\./,
  "website must declare lucide-react for hero CTA icons",
);
assert.doesNotMatch(
  page,
  /use client/,
  "home page must stay a Server Component",
);
assert.match(page, /from "next\/image"/, "hero must use next/image for icon");
assert.match(page, /from "lucide-react"/, "hero must use lucide icons");
assert.match(page, /src="\/icon\.png"/, "hero must reuse the project icon");
assert.match(page, /id="hero-title"/, "hero title must be addressable");
assert.match(page, /<HeroProductPreview t=\{t\} \/>/);
assert.match(page, /function HeroProductPreview/);
assert.match(page, /motion-safe:animate-pulse/);
assert.match(page, /href=\{`\/\$\{locale\}\/download`\}/);
assert.match(page, /href="https:\/\/github\.com\/colna\/CCUse"/);
assert.match(page, /<Download aria-hidden="true" \/>/);
assert.match(page, /<Github aria-hidden="true" \/>/);
assert.match(page, /id="features"/, "feature anchor must remain available");
assert.doesNotMatch(
  page,
  /tracking-apple/,
  "new hero typography must not add negative letter spacing utilities",
);

for (const [locale, bundle] of Object.entries(messages)) {
  const home = bundle.HomePage;

  assert.equal(
    home.title,
    "CCUse",
    `${locale} hero H1 must be the product name`,
  );
  assert.ok(home.slogan, `${locale} hero slogan must exist`);
  assert.ok(home.actions.label, `${locale} action nav label must exist`);
  assert.ok(home.actions.download, `${locale} download CTA must exist`);
  assert.ok(home.actions.github, `${locale} GitHub CTA must exist`);
  assert.equal(home.heroMetrics.endpoint.value, "http://127.0.0.1:8787");
  assert.ok(home.heroMetrics.providers.value.includes("OpenAI"));
  assert.ok(home.heroPreview.caption, `${locale} preview caption must exist`);
  assert.ok(home.heroPreview.traffic.client, `${locale} client step missing`);
  assert.ok(home.heroPreview.traffic.proxy, `${locale} proxy step missing`);
  assert.ok(
    home.heroPreview.traffic.provider,
    `${locale} provider step missing`,
  );
  assert.ok(home.heroPreview.providers.anthropic.name);
  assert.ok(home.heroPreview.providers.openai.status);
  assert.ok(home.heroPreview.providers.gemini.status);
}

console.log("Home hero contract passed");
