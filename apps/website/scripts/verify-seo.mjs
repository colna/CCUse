import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.cwd();

function read(relativePath) {
  return readFileSync(path.join(root, relativePath), "utf8");
}

function readBytes(relativePath) {
  return readFileSync(path.join(root, relativePath));
}

function readRepoBytes(relativePath) {
  return readFileSync(path.join(root, "../..", relativePath));
}

function assertPath(relativePath) {
  assert.ok(
    existsSync(path.join(root, relativePath)),
    `${relativePath} must exist`,
  );
}

const layout = read("app/[locale]/layout.tsx");
const sitemap = read("app/sitemap.ts");
const robots = read("app/robots.ts");
const site = read("site.ts");

for (const requiredPath of [
  "app/sitemap.ts",
  "app/robots.ts",
  "public/opengraph-image.png",
  "site.ts",
]) {
  assertPath(requiredPath);
}

assert.match(site, /https:\/\/ccuse\.app/);
assert.match(site, /NEXT_PUBLIC_SITE_URL/);
assert.match(site, /absoluteUrl/);

assert.match(layout, /metadataBase: new URL\(siteUrl\)/);
assert.match(layout, /alternates:/);
assert.match(layout, /canonical/);
assert.match(layout, /"x-default": `\/\$\{defaultLocale\}`/);
assert.match(layout, /openGraph:/);
assert.match(layout, /twitter:/);
assert.match(
  layout,
  /const openGraphImage = absoluteUrl\("\/opengraph-image\.png"\)/,
);
assert.match(layout, /url: openGraphImage/);
assert.match(layout, /images: \[openGraphImage\]/);
assert.match(layout, /card: "summary"/);

assert.match(sitemap, /MetadataRoute\.Sitemap/);
assert.match(sitemap, /locales\.flatMap/);
assert.match(sitemap, /getRegisteredDocsSlugs/);
assert.match(sitemap, /absoluteUrl\(`\/\$\{locale\}`\)/);
assert.match(sitemap, /absoluteUrl\(`\/\$\{locale\}\/docs`\)/);
assert.match(sitemap, /absoluteUrl\(`\/\$\{locale\}\/features`\)/);
assert.match(sitemap, /absoluteUrl\(`\/\$\{locale\}\/download`\)/);
assert.match(sitemap, /absoluteUrl\(`\/\$\{locale\}\/docs\/\$\{slug\}`\)/);
assert.match(sitemap, /changeFrequency: "weekly"/);

assert.match(robots, /MetadataRoute\.Robots/);
assert.match(robots, /userAgent: "\*"/);
assert.match(robots, /allow: "\/"/);
assert.match(robots, /absoluteUrl\("\/sitemap\.xml"\)/);

assert.deepEqual(
  readBytes("public/opengraph-image.png"),
  readRepoBytes("docs/icon.png"),
  "website Open Graph image must reuse the project icon source",
);

console.log("Next.js SEO contract passed");
