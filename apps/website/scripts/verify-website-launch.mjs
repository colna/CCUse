import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
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

function assertPath(relativePath) {
  assert.ok(
    existsSync(path.join(root, relativePath)),
    `${relativePath} must exist`,
  );
}

for (const requiredPath of [
  "app/[locale]/legal/privacy/page.tsx",
  "app/[locale]/legal/terms/page.tsx",
  "components/mobile-navigation.tsx",
  "components/site-analytics.tsx",
  "vercel.json",
]) {
  assertPath(requiredPath);
}

const layout = read("app/[locale]/layout.tsx");
const header = read("components/site-header.tsx");
const footer = read("components/site-footer.tsx");
const mobileNav = read("components/mobile-navigation.tsx");
const analytics = read("components/site-analytics.tsx");
const site = read("site.ts");
const sitemap = read("app/sitemap.ts");
const vercelConfig = readJson("vercel.json");
const checklist = readRepo("docs/website-launch-checklist.md");
const messages = {
  en: readJson("messages/en.json"),
  zh: readJson("messages/zh.json"),
};

assert.match(layout, /<SiteAnalytics \/>/);
assert.match(analytics, /next\/script/);
assert.match(analytics, /strategy="afterInteractive"/);
assert.match(site, /NEXT_PUBLIC_PLAUSIBLE_DOMAIN/);
assert.match(site, /NEXT_PUBLIC_PLAUSIBLE_SRC/);
assert.match(site, /VERCEL_PROJECT_PRODUCTION_URL/);
assert.match(site, /VERCEL_URL/);

assert.match(header, /MobileNavigation/);
assert.match(mobileNav, /sm:hidden/);
assert.match(mobileNav, /aria-expanded=\{open\}/);
assert.match(mobileNav, /setOpen\(\(current\) => !current\)/);
assert.match(mobileNav, /onClick=\{\(\) => setOpen\(false\)\}/);

assert.match(footer, /\/legal\/privacy/);
assert.match(footer, /\/legal\/terms/);
assert.match(footer, /\/download\/preview/);
assert.match(sitemap, /absoluteUrl\(`\/\$\{locale\}\/legal\/privacy`\)/);
assert.match(sitemap, /absoluteUrl\(`\/\$\{locale\}\/legal\/terms`\)/);

assert.equal(vercelConfig.framework, "nextjs");
assert.match(vercelConfig.buildCommand, /@ccuse\/website build/);
assert.match(vercelConfig.installCommand, /pnpm install --frozen-lockfile/);
assert.equal(vercelConfig.outputDirectory, ".next");
assert.ok(
  vercelConfig.headers.some((entry) =>
    entry.headers.some((header) => header.key === "Strict-Transport-Security"),
  ),
  "Vercel headers must include HSTS",
);

assert.match(checklist, /Vercel Project Settings/);
assert.match(checklist, /No GitHub Actions deploy workflow/);
assert.match(checklist, /NEXT_PUBLIC_SITE_URL/);
assert.match(checklist, /Root directory/);

for (const [locale, bundle] of Object.entries(messages)) {
  assert.ok(bundle.Footer.links.preview, `${locale} preview footer link`);
  assert.ok(bundle.Footer.links.privacy, `${locale} privacy footer link`);
  assert.ok(bundle.Footer.links.terms, `${locale} terms footer link`);
  assert.ok(bundle.Navigation.mobile.open, `${locale} mobile open label`);
  assert.ok(
    bundle.LegalPage.privacy.sections.localData.body,
    `${locale} privacy`,
  );
  assert.ok(bundle.LegalPage.terms.sections.previews.body, `${locale} terms`);
}

console.log("Website launch contract passed");
