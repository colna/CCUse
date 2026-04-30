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

function readJson(relativePath) {
  return JSON.parse(read(relativePath));
}

function assertPath(relativePath) {
  assert.ok(
    existsSync(path.join(root, relativePath)),
    `${relativePath} must exist`,
  );
}

const packageJson = readJson("package.json");
const layout = read("app/[locale]/layout.tsx");
const page = read("app/[locale]/page.tsx");
const header = read("components/site-header.tsx");
const footer = read("components/site-footer.tsx");
const themeProvider = read("components/theme-provider.tsx");
const themeToggle = read("components/theme-toggle.tsx");
const config = read("next.config.mjs");

function pngInfo(relativePath) {
  const bytes = readBytes(relativePath);
  assert.equal(bytes.subarray(0, 8).toString("binary"), "\x89PNG\r\n\x1a\n");
  assert.equal(bytes.subarray(12, 16).toString("ascii"), "IHDR");
  return {
    width: bytes.readUInt32BE(16),
    height: bytes.readUInt32BE(20),
    bitDepth: bytes[24],
    colorType: bytes[25],
  };
}

function icoSizes(relativePath) {
  const bytes = readBytes(relativePath);
  assert.equal(bytes.readUInt16LE(0), 0);
  assert.equal(bytes.readUInt16LE(2), 1);
  const count = bytes.readUInt16LE(4);
  const sizes = [];
  for (let index = 0; index < count; index += 1) {
    const offset = 6 + index * 16;
    const width = bytes[offset] === 0 ? 256 : bytes[offset];
    const height = bytes[offset + 1] === 0 ? 256 : bytes[offset + 1];
    const imageLength = bytes.readUInt32LE(offset + 8);
    const imageOffset = bytes.readUInt32LE(offset + 12);
    assert.equal(width, height);
    assert.equal(
      bytes.subarray(imageOffset, imageOffset + 8).toString("binary"),
      "\x89PNG\r\n\x1a\n",
    );
    assert.ok(imageOffset + imageLength <= bytes.length);
    sizes.push(width);
  }
  return sizes;
}

assert.equal(packageJson.name, "@ccuse/website");
assert.equal(packageJson.private, true);
assert.equal(packageJson.scripts.build, "next build");
assert.equal(packageJson.scripts.typecheck, "tsc --noEmit");
assert.match(packageJson.dependencies?.next ?? "", /^\^?14\./);
assert.match(packageJson.dependencies?.["next-intl"] ?? "", /^\^?4\./);
assert.match(packageJson.dependencies?.react ?? "", /^\^?18\./);
assert.match(packageJson.dependencies?.["react-dom"] ?? "", /^\^?18\./);

for (const requiredPath of [
  "app/[locale]/layout.tsx",
  "app/[locale]/page.tsx",
  "components/site-header.tsx",
  "components/site-footer.tsx",
  "components/theme-provider.tsx",
  "components/theme-toggle.tsx",
  "app/icon.png",
  "app/apple-icon.png",
  "app/favicon.ico",
  "app/sitemap.ts",
  "app/robots.ts",
  "public/opengraph-image.png",
  "i18n/request.ts",
  "i18n/routing.ts",
  "messages/en.json",
  "messages/zh.json",
  "middleware.ts",
  "next.config.mjs",
  "next-env.d.ts",
  "tsconfig.json",
]) {
  assertPath(requiredPath);
}

assert.match(
  layout,
  /export async function generateMetadata/,
  "localized root layout must export Metadata API config",
);
assert.match(
  layout,
  /export const viewport/,
  "root layout must export viewport config",
);
assert.doesNotMatch(
  layout,
  /use client/,
  "root layout must stay a Server Component",
);
assert.doesNotMatch(
  page,
  /use client/,
  "home page must stay a Server Component",
);
assert.doesNotMatch(
  header,
  /use client/,
  "site header must stay a Server Component",
);
assert.doesNotMatch(
  footer,
  /use client/,
  "site footer must stay a Server Component",
);
assert.match(themeProvider, /use client/);
assert.match(themeToggle, /use client/);
assert.match(layout, /<SiteHeader locale=\{locale\} \/>/);
assert.match(layout, /<SiteFooter locale=\{locale\} \/>/);
assert.match(layout, /<ThemeProvider>/);
assert.match(header, /<ThemeToggle/);
assert.match(header, /src="\/icon\.png"/, "header logo must use app icon");
assert.match(footer, /src="\/icon\.png"/, "footer logo must use app icon");
assert.match(
  header,
  /languages\.\$\{item\}/,
  "header must expose language switch",
);
assert.match(
  page,
  /href=\{`\/\$\{locale\}\/download`\}/,
  "home page must expose a locale-aware download CTA",
);
assert.match(
  config,
  /reactStrictMode:\s*true/,
  "Next config must keep React strict mode",
);
assert.match(
  config,
  /next-intl\/plugin/,
  "Next config must install the next-intl plugin",
);

const websiteIcon = readBytes("app/icon.png");
const masterIcon = readRepoBytes("docs/icon.png");
assert.deepEqual(
  websiteIcon,
  masterIcon,
  "website app/icon.png must be copied from docs/icon.png",
);
assert.deepEqual(
  readBytes("public/opengraph-image.png"),
  masterIcon,
  "website Open Graph image must be copied from docs/icon.png",
);

assert.deepEqual(pngInfo("app/icon.png"), {
  width: 801,
  height: 801,
  bitDepth: 8,
  colorType: 6,
});
assert.deepEqual(pngInfo("app/apple-icon.png"), {
  width: 180,
  height: 180,
  bitDepth: 8,
  colorType: 6,
});
assert.deepEqual(icoSizes("app/favicon.ico"), [16, 32, 48]);

console.log("Next.js website contract passed");
