import { spawn } from "node:child_process";
import { once } from "node:events";
import { setTimeout as delay } from "node:timers/promises";

import { expect, test, type Page } from "@playwright/test";

interface FixtureConfig {
  control_base_url: string;
  proxy_base_url: string;
  api_key: string;
  mock_provider_base_url: string;
}

interface RunningFixture {
  config: FixtureConfig;
  stop: () => Promise<void>;
}

function readFirstStdoutLine(
  child: ReturnType<typeof spawn>,
  stderr: string[],
): Promise<string> {
  return new Promise((resolve, reject) => {
    let buffer = "";
    const onData = (chunk: Buffer) => {
      buffer += chunk.toString("utf8");
      const newline = buffer.indexOf("\n");
      if (newline === -1) return;
      child.stdout.off("data", onData);
      resolve(buffer.slice(0, newline).trim());
    };
    child.stdout.on("data", onData);
    child.once("exit", (code, signal) => {
      reject(
        new Error(
          `fixture exited before ready (code=${code}, signal=${signal}): ${stderr.join("")}`,
        ),
      );
    });
    child.once("error", reject);
  });
}

async function startFixture(): Promise<RunningFixture> {
  const stderr: string[] = [];
  const child = spawn(
    "cargo",
    [
      "run",
      "--quiet",
      "--manifest-path",
      "src-tauri/Cargo.toml",
      "--example",
      "e2e_fixture",
    ],
    {
      cwd: process.cwd(),
      stdio: ["ignore", "pipe", "pipe"],
    },
  );
  child.stderr.on("data", (chunk: Buffer) =>
    stderr.push(chunk.toString("utf8")),
  );

  const readyLine = await Promise.race([
    readFirstStdoutLine(child, stderr),
    delay(90_000).then(() => {
      throw new Error(`fixture did not start within 90s: ${stderr.join("")}`);
    }),
  ]);
  const config = JSON.parse(readyLine) as FixtureConfig;

  return {
    config,
    stop: async () => {
      if (!child.killed) child.kill();
      await Promise.race([once(child, "exit"), delay(5_000)]);
    },
  };
}

async function installTauriMock(page: Page, fixture: FixtureConfig) {
  await page.addInitScript((config: FixtureConfig) => {
    window.localStorage.setItem("i18nextLng", "zh");

    type InvokeArgs = Record<string, unknown> | undefined;
    type Callback = (event: unknown) => void;
    type TauriWindow = Window & {
      __TAURI_INTERNALS__: {
        callbacks: Map<number, Callback>;
        invoke: (cmd: string, args?: InvokeArgs) => Promise<unknown>;
        transformCallback: (callback: Callback) => number;
        unregisterCallback: (id: number) => void;
      };
      __TAURI_EVENT_PLUGIN_INTERNALS__: {
        unregisterListener: () => void;
      };
    };

    const fetchJson = async (path: string, init?: RequestInit) => {
      const response = await fetch(`${config.control_base_url}${path}`, {
        ...init,
        headers: {
          "content-type": "application/json",
          ...(init?.headers ?? {}),
        },
      });
      if (!response.ok) {
        throw new Error(await response.text());
      }
      return response.json();
    };

    const tauriWindow = window as TauriWindow;
    const callbacks = new Map<number, Callback>();
    let nextCallbackId = 1;

    tauriWindow.__TAURI_INTERNALS__ = {
      callbacks,
      invoke: async (cmd: string, args?: InvokeArgs) => {
        switch (cmd) {
          case "list_providers":
            return fetchJson("/providers");
          case "add_provider":
            return fetchJson("/providers", {
              method: "POST",
              body: JSON.stringify((args as { input: unknown }).input),
            });
          case "get_local_api_config":
          case "regenerate_api_key":
          case "restart_proxy":
            return {
              base_url: config.proxy_base_url,
              api_key: config.api_key,
            };
          case "get_health_snapshot": {
            const providers = (await fetchJson("/providers")) as {
              id: string;
              name: string;
            }[];
            return {
              providers: providers.map((provider) => ({
                provider_id: provider.id,
                provider_name: provider.name,
                status: "healthy",
                success_rate: 1,
                response_time_us: null,
              })),
            };
          }
          case "get_metrics_timeseries":
            return fetchJson("/metrics");
          case "get_provider_cost_summary":
          case "get_switch_timeline":
            return [];
          case "plugin:event|listen":
            return 1;
          case "plugin:event|unlisten":
            return null;
          default:
            throw new Error(`Unhandled Tauri command: ${cmd}`);
        }
      },
      transformCallback: (callback: Callback) => {
        const id = nextCallbackId;
        nextCallbackId += 1;
        callbacks.set(id, callback);
        return id;
      },
      unregisterCallback: (id: number) => {
        callbacks.delete(id);
      },
    };
    tauriWindow.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
      unregisterListener: () => undefined,
    };
  }, fixture);
}

test("UI provider add drives a real proxy request into dashboard metrics", async ({
  page,
  request,
}) => {
  const fixture = await startFixture();
  try {
    await installTauriMock(page, fixture.config);

    await page.goto("/#/providers");
    await page.getByLabel("名称").fill("E2E Mock OpenAI");
    await page
      .getByLabel("Base URL")
      .fill(fixture.config.mock_provider_base_url);
    await page.getByLabel("API Key").fill("sk-upstream-e2e");
    await page.getByLabel("优先级").fill("1");
    await page.getByRole("button", { name: "添加" }).click();
    await expect(page.getByText("E2E Mock OpenAI")).toBeVisible();

    const response = await request.post(
      `${fixture.config.proxy_base_url}/v1/chat/completions`,
      {
        headers: {
          authorization: `Bearer ${fixture.config.api_key}`,
        },
        data: {
          model: "gpt-4o",
          messages: [{ role: "user", content: "ping" }],
          stream: false,
        },
      },
    );
    expect(response.status()).toBe(200);

    await page.goto("/#/dashboard");
    const requestsCard = page
      .getByText("今日请求", { exact: true })
      .locator("xpath=ancestor::div[contains(@class,'rounded-xl')][1]");
    await expect(requestsCard.getByText("1", { exact: true })).toBeVisible();
  } finally {
    await fixture.stop();
  }
});
