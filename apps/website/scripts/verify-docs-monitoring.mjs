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
const enGuide = read("content/docs/en/monitoring-alerts.mdx");
const zhGuide = read("content/docs/zh/monitoring-alerts.mdx");
const monitorCommand = readRepo(
  "apps/desktop/src-tauri/src/commands/monitor.rs",
);
const statusCards = readRepo(
  "apps/desktop/src/components/dashboard/StatusCards.tsx",
);
const switchTimeline = readRepo(
  "apps/desktop/src/components/dashboard/SwitchTimeline.tsx",
);
const tauriApi = readRepo("apps/desktop/src/lib/tauri.ts");

for (const requiredPath of [
  "content/docs/en/monitoring-alerts.mdx",
  "content/docs/zh/monitoring-alerts.mdx",
]) {
  assertPath(requiredPath);
}

assert.match(packageJson.scripts.test, /verify-docs-monitoring\.mjs/);
assert.match(docsContent, /EnMonitoringAlerts/);
assert.match(docsContent, /ZhMonitoringAlerts/);
assert.match(docsContent, /"monitoring-alerts": EnMonitoringAlerts/);
assert.match(docsContent, /"monitoring-alerts": ZhMonitoringAlerts/);

assert.match(monitorCommand, /get_metrics_timeseries/);
assert.match(monitorCommand, /get_provider_cost_summary/);
assert.match(monitorCommand, /get_switch_timeline/);
assert.match(monitorCommand, /LIMIT 50/);
assert.match(monitorCommand, /strftime\('%Y-%m-%dT%H:'/);
assert.match(statusCards, /current_provider/);
assert.match(statusCards, /today_requests/);
assert.match(statusCards, /success_rate/);
assert.match(statusCards, /avg_response_time/);
assert.match(switchTimeline, /switch_timeline_title/);
assert.match(tauriApi, /EVENT_PROVIDER_STATUS_CHANGED/);
assert.match(tauriApi, /sendNotification/);

for (const source of [enGuide, zhGuide]) {
  assert.match(source, /^---\ntitle: /m);
  assert.match(source, /\nsection: /);
  assert.match(source, /\norder: 4/);
  assert.match(source, /24/);
  assert.match(source, /5-minute|5 分钟/);
  assert.match(source, /P95/);
  assert.match(source, /cost_per_1k_tokens/);
  assert.match(source, /switch_history/);
  assert.match(source, /LIMIT 50|50/);
  assert.match(source, /provider-status-changed/);
  assert.match(source, /sk-local-\.\.\./);
}

assert.match(enGuide, /Status Cards/);
assert.match(enGuide, /Success Rate/);
assert.match(enGuide, /Response Time/);
assert.match(enGuide, /Cost by Provider/);
assert.match(enGuide, /Switch Timeline/);
assert.match(enGuide, /Alert Signals/);

assert.match(zhGuide, /状态卡片/);
assert.match(zhGuide, /成功率/);
assert.match(zhGuide, /响应时间/);
assert.match(zhGuide, /供应商成本/);
assert.match(zhGuide, /切换时间线/);
assert.match(zhGuide, /告警信号/);

assert.match(
  enIndex,
  /\[Monitoring and Alerts\]\(\/en\/docs\/monitoring-alerts\)/,
);
assert.match(zhIndex, /\[监控与告警\]\(\/zh\/docs\/monitoring-alerts\)/);

console.log("Docs monitoring contract passed");
