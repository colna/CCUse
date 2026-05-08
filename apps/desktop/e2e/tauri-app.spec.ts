import {
  createServer,
  type IncomingMessage,
  type ServerResponse,
} from "node:http";
import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";

import { expect, test, type APIRequestContext } from "@playwright/test";

const WEBDRIVER_URL = "http://127.0.0.1:4444";
const ELEMENT_KEY = "element-6066-11e4-a52e-4f735466cecf";

interface WebDriverElement {
  [ELEMENT_KEY]: string;
}

interface WebDriverResponse<T> {
  value: T;
}

interface MockProvider {
  baseUrl: string;
  stop: () => Promise<void>;
}

class TauriDriverSession {
  private constructor(private readonly sessionId: string) {}

  static async create(application: string): Promise<TauriDriverSession> {
    const response = await webdriverRequest<{ sessionId: string }>(
      "POST",
      "/session",
      {
        capabilities: {
          alwaysMatch: {
            browserName: "wry",
            "tauri:options": {
              application,
            },
          },
        },
      },
    );

    return new TauriDriverSession(response.sessionId);
  }

  async quit(): Promise<void> {
    await webdriverRequest("DELETE", `/session/${this.sessionId}`);
  }

  async execute<T>(script: string, args: unknown[] = []): Promise<T> {
    return webdriverRequest<T>(
      "POST",
      `/session/${this.sessionId}/execute/sync`,
      {
        script,
        args,
      },
    );
  }

  async findCss(selector: string): Promise<WebDriverElement> {
    return webdriverRequest<WebDriverElement>(
      "POST",
      `/session/${this.sessionId}/element`,
      {
        using: "css selector",
        value: selector,
      },
    );
  }

  async clickCss(selector: string): Promise<void> {
    const element = await this.findCss(selector);
    await webdriverRequest(
      "POST",
      `/session/${this.sessionId}/element/${element[ELEMENT_KEY]}/click`,
      {},
    );
  }

  async setInputValue(selector: string, value: string): Promise<void> {
    await this.execute(
      `
        const input = document.querySelector(arguments[0]);
        if (!(input instanceof HTMLInputElement)) {
          throw new Error('input not found: ' + arguments[0]);
        }
        const descriptor = Object.getOwnPropertyDescriptor(
          window.HTMLInputElement.prototype,
          'value'
        );
        descriptor.set.call(input, arguments[1]);
        input.dispatchEvent(new Event('input', { bubbles: true }));
        input.dispatchEvent(new Event('change', { bubbles: true }));
      `,
      [selector, value],
    );
  }

  async textContent(selector: string): Promise<string | null> {
    return this.execute<string | null>(
      `
        const element = document.querySelector(arguments[0]);
        return element ? element.textContent : null;
      `,
      [selector],
    );
  }

  async waitForElement(selector: string, timeoutMs = 15_000): Promise<void> {
    await waitFor(
      async () => {
        const exists = await this.execute<boolean>(
          "return Boolean(document.querySelector(arguments[0]));",
          [selector],
        );
        return exists;
      },
      `element ${selector}`,
      timeoutMs,
    );
  }

  async waitForText(
    selector: string,
    expected: string,
    timeoutMs = 15_000,
  ): Promise<void> {
    await waitFor(
      async () => {
        const text = await this.textContent(selector);
        return text?.includes(expected) ?? false;
      },
      `text ${expected} in ${selector}`,
      timeoutMs,
    );
  }

  async waitForTextMatching(
    selector: string,
    pattern: RegExp,
    timeoutMs = 15_000,
  ): Promise<string> {
    let latest = "";
    await waitFor(
      async () => {
        latest = (await this.textContent(selector))?.trim() ?? "";
        return pattern.test(latest);
      },
      `text matching ${pattern} in ${selector}`,
      timeoutMs,
    );
    return latest;
  }
}

async function webdriverRequest<T>(
  method: "DELETE" | "GET" | "POST",
  endpoint: string,
  body?: unknown,
): Promise<T> {
  const response = await fetch(`${WEBDRIVER_URL}${endpoint}`, {
    method,
    headers: body ? { "content-type": "application/json" } : undefined,
    body: body ? JSON.stringify(body) : undefined,
  });
  const payload = (await response
    .json()
    .catch(() => null)) as WebDriverResponse<T> | null;
  if (!response.ok || payload == null) {
    throw new Error(
      `WebDriver ${method} ${endpoint} failed with ${response.status}: ${JSON.stringify(payload)}`,
    );
  }
  return payload.value;
}

async function waitFor(
  predicate: () => Promise<boolean>,
  label: string,
  timeoutMs: number,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  let lastError: unknown = null;
  while (Date.now() < deadline) {
    try {
      if (await predicate()) return;
    } catch (err) {
      lastError = err;
    }
    await delay(250);
  }
  throw new Error(
    `Timed out waiting for ${label}${lastError ? `: ${String(lastError)}` : ""}`,
  );
}

async function startTauriDriver(appPath: string): Promise<{
  process: ChildProcessWithoutNullStreams;
  tempRoot: string;
}> {
  const tempRoot = await mkdtemp(path.join(tmpdir(), "ccuse-tauri-e2e-"));
  const driver = spawn(process.env.CCUSE_TAURI_DRIVER ?? "tauri-driver", [], {
    env: {
      ...process.env,
      APPDATA: path.join(tempRoot, "appdata"),
      HOME: path.join(tempRoot, "home"),
      LOCALAPPDATA: path.join(tempRoot, "localappdata"),
      XDG_CACHE_HOME: path.join(tempRoot, "xdg-cache"),
      XDG_CONFIG_HOME: path.join(tempRoot, "xdg-config"),
      XDG_DATA_HOME: path.join(tempRoot, "xdg-data"),
    },
    stdio: ["ignore", "pipe", "pipe"],
  });
  const stderr: string[] = [];
  driver.stderr.on("data", (chunk: Buffer) =>
    stderr.push(chunk.toString("utf8")),
  );

  await waitFor(
    async () => {
      if (driver.exitCode !== null) {
        throw new Error(`tauri-driver exited early: ${stderr.join("")}`);
      }
      try {
        await fetch(`${WEBDRIVER_URL}/status`);
        return true;
      } catch {
        return false;
      }
    },
    "tauri-driver /status",
    15_000,
  );

  expect(appPath).toBeTruthy();
  return { process: driver, tempRoot };
}

async function stopTauriDriver(
  driver: ChildProcessWithoutNullStreams,
): Promise<void> {
  if (driver.exitCode !== null) return;
  driver.kill();
  await Promise.race([
    new Promise<void>((resolve) => driver.once("exit", () => resolve())),
    delay(5_000),
  ]);
}

function startMockProvider(): Promise<MockProvider> {
  const server = createServer(
    (request: IncomingMessage, response: ServerResponse) => {
      if (request.method === "GET" && request.url === "/v1/models") {
        response.writeHead(200, { "content-type": "application/json" });
        response.end(
          JSON.stringify({
            object: "list",
            data: [{ id: "gpt-5.5-instant", object: "model" }],
          }),
        );
        return;
      }

      if (request.method === "POST" && request.url === "/v1/chat/completions") {
        response.writeHead(200, { "content-type": "application/json" });
        response.end(
          JSON.stringify({
            id: "chatcmpl-tauri-e2e",
            object: "chat.completion",
            created: 1_700_000_000,
            model: "gpt-5.5-instant",
            choices: [
              {
                index: 0,
                message: { role: "assistant", content: "pong from tauri" },
                finish_reason: "stop",
              },
            ],
            usage: { prompt_tokens: 4, completion_tokens: 3, total_tokens: 7 },
          }),
        );
        return;
      }

      response.writeHead(404, { "content-type": "text/plain" });
      response.end("not found");
    },
  );

  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      if (address == null || typeof address === "string") {
        reject(new Error("mock provider did not bind to a TCP address"));
        return;
      }
      resolve({
        baseUrl: `http://127.0.0.1:${address.port}`,
        stop: () =>
          new Promise<void>((stopResolve, stopReject) => {
            server.close((err) => (err ? stopReject(err) : stopResolve()));
          }),
      });
    });
  });
}

async function addProvider(
  session: TauriDriverSession,
  mockProviderBaseUrl: string,
): Promise<void> {
  await session.execute(`
    window.localStorage.setItem('i18nextLng', 'zh');
    window.location.hash = '#/providers';
  `);
  await session.waitForElement("#provider-name");
  await session.setInputValue("#provider-name", "Tauri E2E OpenAI");
  await session.setInputValue("#provider-base-url", mockProviderBaseUrl);
  await session.setInputValue("#provider-api-key", "sk-upstream-tauri-e2e");
  await session.setInputValue("#provider-priority", "1");
  await session.clickCss("form button[type='submit']");
  await session.waitForText("body", "Tauri E2E OpenAI");
}

async function readLocalApiConfig(session: TauriDriverSession): Promise<{
  apiKey: string;
  baseUrl: string;
}> {
  await session.execute("window.location.hash = '#/dashboard';");
  await session.waitForElement("[data-testid='local-api-card']");
  const baseUrl = await session.waitForTextMatching(
    "[data-testid='local-api-base-url']",
    /^http:\/\/127\.0\.0\.1:\d+$/,
  );
  await session.clickCss("[data-testid='local-api-toggle-key']");
  const apiKey = await session.waitForTextMatching(
    "[data-testid='local-api-key']",
    /^sk-local-[A-Za-z0-9]+$/,
  );

  return { apiKey, baseUrl };
}

async function callLocalProxy(
  request: APIRequestContext,
  baseUrl: string,
  apiKey: string,
): Promise<void> {
  const response = await request.post(`${baseUrl}/v1/chat/completions`, {
    headers: { authorization: `Bearer ${apiKey}` },
    data: {
      model: "gpt-5.5-instant",
      messages: [{ role: "user", content: "ping" }],
      stream: false,
    },
  });

  expect(response.status()).toBe(200);
  const body = (await response.json()) as {
    choices?: Array<{ message?: { content?: string } }>;
  };
  expect(body.choices?.[0]?.message?.content).toBe("pong from tauri");
}

test.describe("Tauri desktop shell", () => {
  test.skip(
    process.platform === "darwin",
    "tauri-driver desktop WebDriver is only available on Linux and Windows",
  );
  test.skip(
    process.env.CCUSE_TAURI_E2E !== "1",
    "set CCUSE_TAURI_E2E=1 and CCUSE_TAURI_APP_PATH to run the real Tauri app",
  );

  test("adds a provider, sends a real local proxy request, and updates dashboard metrics", async ({
    request,
  }) => {
    const appPath = process.env.CCUSE_TAURI_APP_PATH;
    expect(
      appPath,
      "CCUSE_TAURI_APP_PATH must point at the built desktop binary",
    ).toBeTruthy();

    const mockProvider = await startMockProvider();
    const driver = await startTauriDriver(appPath ?? "");
    let session: TauriDriverSession | null = null;

    try {
      session = await TauriDriverSession.create(appPath ?? "");
      await addProvider(session, mockProvider.baseUrl);
      const config = await readLocalApiConfig(session);
      await callLocalProxy(request, config.baseUrl, config.apiKey);

      await session.execute(`
        window.location.hash = '#/providers';
        window.location.hash = '#/dashboard';
      `);
      await session.waitForText(
        "[data-testid='today-requests-card-value']",
        "1",
        15_000,
      );
    } finally {
      await session?.quit().catch(() => undefined);
      await stopTauriDriver(driver.process);
      await mockProvider.stop().catch(() => undefined);
      await rm(driver.tempRoot, { recursive: true, force: true });
    }
  });
});
