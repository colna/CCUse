import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.cwd();
const repoRoot = path.join(root, "../..");

function read(relativePath) {
  return readFileSync(path.join(root, relativePath), "utf8");
}

function readRepo(relativePath) {
  return readFileSync(path.join(repoRoot, relativePath), "utf8");
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
const docsContent = read("lib/docs-content.ts");
const enIndex = read("content/docs/en/index.mdx");
const zhIndex = read("content/docs/zh/index.mdx");
const enFaq = read("content/docs/en/faq-troubleshooting.mdx");
const zhFaq = read("content/docs/zh/faq-troubleshooting.mdx");
const rootFaq = readRepo("docs/FAQ.md");
const proxyRuntime = readRepo("apps/desktop/src-tauri/src/proxy/runtime.rs");
const tauriConfig = readJson("../../apps/desktop/src-tauri/tauri.conf.json");
const releaseWorkflow = readRepo(".github/workflows/release.yml");

for (const requiredPath of [
  "content/docs/en/faq-troubleshooting.mdx",
  "content/docs/zh/faq-troubleshooting.mdx",
]) {
  assertPath(requiredPath);
}

assert.match(packageJson.scripts.test, /verify-docs-faq\.mjs/);
assert.match(docsContent, /EnFaqTroubleshooting/);
assert.match(docsContent, /ZhFaqTroubleshooting/);
assert.match(docsContent, /"faq-troubleshooting": EnFaqTroubleshooting/);
assert.match(docsContent, /"faq-troubleshooting": ZhFaqTroubleshooting/);

assert.match(proxyRuntime, /DEFAULT_PROXY_PORT: u16 = 8787/);
assert.match(proxyRuntime, /DEFAULT_PROXY_ATTEMPTS: u16 = 100/);
assert.equal(
  tauriConfig.bundle.windows.webviewInstallMode.type,
  "downloadBootstrapper",
);
assert.match(releaseWorkflow, /CCUse_\$\{VERSION\}_aarch64\.dmg/);
assert.match(releaseWorkflow, /CCUse_\$\{VERSION\}_x64\.dmg/);
assert.match(releaseWorkflow, /CCUse_\$\{VERSION\}_x64-setup\.exe/);
assert.match(releaseWorkflow, /APPLE_SIGNING_ENABLED/);

for (const source of [enFaq, zhFaq, rootFaq]) {
  assert.match(source, /8787/);
  assert.match(source, /8886/);
  assert.doesNotMatch(source, /8080-8180/);
  assert.match(source, /WebView2/);
  assert.match(source, /downloadBootstrapper/);
  assert.match(source, /CCUse_<version>_x64-setup\.exe/);
  assert.match(source, /CCUse_<version>_aarch64\.dmg/);
  assert.match(source, /CCUse_<version>_x64\.dmg/);
  assert.match(source, /SmartScreen|Defender|误报/);
  assert.match(source, /sk-local-\.\.\./);
  assert.match(source, /providers_not_configured/);
}

assert.match(enFaq, /Local Proxy Port Is Busy/);
assert.match(enFaq, /WebView2 Is Missing/);
assert.match(enFaq, /macOS Says The App Cannot Be Verified/);
assert.match(enFaq, /Windows SmartScreen or Defender Warns/);
assert.match(enFaq, /401 Unauthorized/);

assert.match(zhFaq, /本地代理端口被占用/);
assert.match(zhFaq, /Windows 缺少 WebView2/);
assert.match(zhFaq, /macOS 提示无法验证开发者/);
assert.match(zhFaq, /Windows SmartScreen 或 Defender 误报/);
assert.match(zhFaq, /401 Unauthorized/);

assert.match(
  enIndex,
  /\[FAQ and Troubleshooting\]\(\/en\/docs\/faq-troubleshooting\)/,
);
assert.match(zhIndex, /\[FAQ 与故障排查\]\(\/zh\/docs\/faq-troubleshooting\)/);

console.log("Docs FAQ contract passed");
