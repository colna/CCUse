import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.cwd();

function read(relativePath) {
  return readFileSync(path.join(root, relativePath), "utf8");
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
  "app/[locale]/download/preview/page.tsx",
]) {
  assertPath(requiredPath);
}

const header = read("components/site-header.tsx");
const footer = read("components/site-footer.tsx");
const messagesEn = read("messages/en.json");
const messagesZh = read("messages/zh.json");

assert.match(header, /legal\/privacy/);
assert.match(header, /legal\/terms/);
assert.match(footer, /legal\/privacy/);
assert.match(footer, /legal\/terms/);
assert.match(messagesEn, /PrivacyPage/);
assert.match(messagesEn, /TermsPage/);
assert.match(messagesEn, /DownloadPreviewPage/);
assert.match(messagesZh, /PrivacyPage/);
assert.match(messagesZh, /TermsPage/);
assert.match(messagesZh, /DownloadPreviewPage/);

console.log("Legal page contract passed");
