import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/tauri", () => ({
  getLocalApiConfig: vi.fn(),
  regenerateApiKey: vi.fn(),
  restartProxy: vi.fn(),
  copyToClipboard: vi.fn(),
  onLocalApiConfigChanged: vi.fn(),
}));

import {
  copyToClipboard,
  getLocalApiConfig,
  onLocalApiConfigChanged,
  regenerateApiKey,
  restartProxy,
  type LocalApiConfig,
} from "@/lib/tauri";
import { LocalApiCard } from "../LocalApiCard";

let lastEventCallback: ((config: LocalApiConfig) => void) | null = null;

const SAMPLE = {
  base_url: "http://127.0.0.1:8787",
  api_key: "sk-local-abcdefghijklmnopqrstuvwxyzABCD12",
};

beforeEach(() => {
  vi.mocked(getLocalApiConfig).mockResolvedValue(SAMPLE);
  vi.mocked(regenerateApiKey).mockReset();
  vi.mocked(restartProxy).mockReset();
  vi.mocked(copyToClipboard).mockReset();
  vi.mocked(copyToClipboard).mockResolvedValue(undefined);
  lastEventCallback = null;
  vi.mocked(onLocalApiConfigChanged).mockImplementation(async (cb) => {
    lastEventCallback = cb;
    return () => {
      lastEventCallback = null;
    };
  });
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("LocalApiCard", () => {
  it("loads the local config on mount and renders running status", async () => {
    render(<LocalApiCard />);
    expect(await screen.findByText("运行中")).toBeInTheDocument();
    expect(screen.getByText(SAMPLE.base_url)).toBeInTheDocument();
  });

  it("masks the api key by default and reveals it on toggle", async () => {
    render(<LocalApiCard />);
    await screen.findByText("运行中");
    // Default: full key is NOT in the DOM, masked tail is.
    expect(screen.queryByText(SAMPLE.api_key)).not.toBeInTheDocument();
    expect(screen.getByText("sk-local---------CD12")).toBeInTheDocument();
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "显示 API Key" }));
    expect(await screen.findByText(SAMPLE.api_key)).toBeInTheDocument();
  });

  it("copies the base url to the clipboard", async () => {
    render(<LocalApiCard />);
    await screen.findByText("运行中");
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "复制 Base URL" }));
    expect(copyToClipboard).toHaveBeenCalledWith(SAMPLE.base_url);
    expect(await screen.findByText("Base URL 已复制")).toBeInTheDocument();
  });

  it("triggers regenerate_api_key and reflects the rotated key", async () => {
    render(<LocalApiCard />);
    await screen.findByText("运行中");
    const rotated = {
      base_url: SAMPLE.base_url,
      api_key: "sk-local-zzzzzzzzzzzzzzzzzzzzzzzzzzzzZZZZ",
    };
    vi.mocked(regenerateApiKey).mockResolvedValueOnce(rotated);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "轮换 Key" }));
    await waitFor(() => expect(regenerateApiKey).toHaveBeenCalledTimes(1));
    expect(
      await screen.findByText("sk-local---------ZZZZ"),
    ).toBeInTheDocument();
  });

  it("renders stopped state when get_local_api_config rejects", async () => {
    vi.mocked(getLocalApiConfig).mockRejectedValueOnce(
      new Error("proxy is not running"),
    );
    render(<LocalApiCard />);
    expect(await screen.findByText("未运行")).toBeInTheDocument();
    expect(screen.getByText("proxy is not running")).toBeInTheDocument();
  });

  it("subscribes to local_api_config_changed and re-renders on event", async () => {
    render(<LocalApiCard />);
    await screen.findByText("运行中");
    await waitFor(() =>
      expect(onLocalApiConfigChanged).toHaveBeenCalledTimes(1),
    );
    // Simulate the backend emitting a rotated key.
    const rotated: LocalApiConfig = {
      base_url: SAMPLE.base_url,
      api_key: "sk-local-rrrrrrrrrrrrrrrrrrrrrrrrrrrrRRRR",
    };
    lastEventCallback?.(rotated);
    expect(
      await screen.findByText("sk-local---------RRRR"),
    ).toBeInTheDocument();
  });

  it("calls restart_proxy and refreshes config", async () => {
    render(<LocalApiCard />);
    await screen.findByText("运行中");
    const next = {
      base_url: "http://127.0.0.1:8788",
      api_key: SAMPLE.api_key,
    };
    vi.mocked(restartProxy).mockResolvedValueOnce(next);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /重启服务/ }));
    await waitFor(() => expect(restartProxy).toHaveBeenCalledTimes(1));
    expect(await screen.findByText(next.base_url)).toBeInTheDocument();
  });
});
