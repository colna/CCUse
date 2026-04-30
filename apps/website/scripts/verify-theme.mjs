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

const packageJson = readJson("package.json");
const layout = read("app/[locale]/layout.tsx");
const provider = read("components/theme-provider.tsx");
const toggle = read("components/theme-toggle.tsx");
const header = read("components/site-header.tsx");
const globals = read("app/globals.css");
const en = readJson("messages/en.json");
const zh = readJson("messages/zh.json");

assert.match(packageJson.dependencies?.["next-themes"] ?? "", /^\^?0\./);
assert.match(layout, /suppressHydrationWarning/);
assert.match(layout, /<ThemeProvider>/);

assert.match(provider, /^"use client";/);
assert.match(provider, /next-themes/);
assert.match(provider, /attribute="class"/);
assert.match(provider, /defaultTheme="system"/);
assert.match(provider, /enableSystem/);
assert.match(provider, /storageKey="ccuse-website-theme"/);

assert.match(toggle, /^"use client";/);
assert.match(toggle, /useTheme/);
assert.match(toggle, /setTheme\(mode\)/);
assert.match(
  toggle,
  /const modes: ThemeMode\[\] = \["system", "light", "dark"\]/,
);
assert.match(toggle, /aria-pressed/);

assert.match(header, /<ThemeToggle/);
assert.match(header, /themes\.system/);
assert.match(header, /themes\.light/);
assert.match(header, /themes\.dark/);

assert.match(globals, /\.dark \{/);
assert.match(globals, /--background:\s*220 11% 5%/);

assert.deepEqual(Object.keys(en.Navigation.themes), [
  "system",
  "light",
  "dark",
]);
assert.deepEqual(
  Object.keys(en.Navigation.themes),
  Object.keys(zh.Navigation.themes),
);

console.log("Next.js theme contract passed");
