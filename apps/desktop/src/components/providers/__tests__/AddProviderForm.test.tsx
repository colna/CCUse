import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/tauri", () => ({
  addProvider: vi.fn(),
  testProviderConnection: vi.fn(),
}));

import { addProvider, testProviderConnection } from "@/lib/tauri";
import { AddProviderForm } from "../AddProviderForm";

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

const savedProvider = {
  id: "11111111-2222-3333-4444-555555555555",
  name: "Work",
  kind: "openai" as const,
  base_url: "https://api.openai.com",
  priority: 50,
  enabled: true,
  created_at: "2026-04-29T00:00:00.000Z",
  updated_at: "2026-04-29T00:00:00.000Z",
};

beforeEach(() => {
  vi.mocked(addProvider).mockReset();
  vi.mocked(testProviderConnection).mockReset();
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("AddProviderForm", () => {
  it("submits a valid input and shows the new provider id", async () => {
    vi.mocked(addProvider).mockResolvedValueOnce(savedProvider);

    const onAdded = vi.fn();
    render(<AddProviderForm onAdded={onAdded} />);

    const user = userEvent.setup();
    await user.type(screen.getByLabelText("名称"), "Work");
    await user.clear(screen.getByLabelText("Base URL"));
    await user.type(
      screen.getByLabelText("Base URL"),
      "https://api.openai.com",
    );
    await user.type(screen.getByLabelText("API Key"), "sk-real-1234");
    await user.clear(screen.getByLabelText("优先级"));
    await user.type(screen.getByLabelText("优先级"), "50");
    await user.click(screen.getByRole("button", { name: "添加" }));

    await waitFor(() => expect(addProvider).toHaveBeenCalledTimes(1));
    expect(addProvider).toHaveBeenCalledWith({
      name: "Work",
      kind: "openai",
      base_url: "https://api.openai.com",
      api_key: "sk-real-1234",
      priority: 50,
      enabled: true,
      monthly_quota: null,
      rate_limit_rpm: null,
      cost_per_1k_tokens: null,
    });
    expect(onAdded).toHaveBeenCalledWith(
      "11111111-2222-3333-4444-555555555555",
    );
    expect(await screen.findByText(/已添加.*11111111/)).toBeInTheDocument();
  });

  it("shows a loading state while saving a new provider", async () => {
    const deferred = createDeferred<Awaited<ReturnType<typeof addProvider>>>();
    vi.mocked(addProvider).mockReturnValueOnce(deferred.promise);

    render(<AddProviderForm />);
    const user = userEvent.setup();
    await user.type(screen.getByLabelText("名称"), "Work");
    await user.type(screen.getByLabelText("API Key"), "sk-real-1234");
    await user.click(screen.getByRole("button", { name: "添加" }));

    await waitFor(() => expect(addProvider).toHaveBeenCalledTimes(1));
    const pendingButton = screen.getByRole("button", { name: "添加中..." });
    expect(pendingButton).toBeDisabled();
    expect(screen.getByLabelText("名称")).toBeDisabled();
    expect(screen.getByLabelText("API Key")).toBeDisabled();

    deferred.resolve(savedProvider);

    expect(await screen.findByText(/已添加.*11111111/)).toBeInTheDocument();
  });

  it("shows per-field errors when required values are missing", async () => {
    render(<AddProviderForm />);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "添加" }));
    expect(await screen.findByText("名称不能为空")).toBeInTheDocument();
    expect(screen.getByText("API Key 不能为空")).toBeInTheDocument();
    expect(screen.queryByText("Base URL 格式无效")).not.toBeInTheDocument();
    expect(addProvider).not.toHaveBeenCalled();
  });

  it("enables OpenAI-compatible provider kinds and disables only unsupported native kinds", () => {
    render(<AddProviderForm />);

    expect(screen.getByRole("button", { name: "Gemini" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Relay" })).not.toBeDisabled();
    expect(screen.getByRole("button", { name: "Custom" })).not.toBeDisabled();
    expect(screen.getByRole("button", { name: "OpenAI" })).not.toBeDisabled();
    expect(
      screen.getByRole("button", { name: "Anthropic" }),
    ).not.toBeDisabled();
  });

  it("requires Base URL for custom OpenAI-compatible providers", async () => {
    render(<AddProviderForm />);
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: "Custom" }));
    await user.type(screen.getByLabelText("名称"), "Local Gateway");
    await user.type(screen.getByLabelText("API Key"), "sk-custom");
    await user.click(screen.getByRole("button", { name: "添加" }));

    expect(
      await screen.findByText("此类型供应商需要填写 Base URL"),
    ).toBeInTheDocument();
    expect(addProvider).not.toHaveBeenCalled();
  });

  it("rejects malformed Base URL", async () => {
    render(<AddProviderForm />);
    const user = userEvent.setup();
    const url = screen.getByLabelText("Base URL");
    await user.clear(url);
    await user.type(url, "not-a-url");
    await user.type(screen.getByLabelText("名称"), "X");
    await user.type(screen.getByLabelText("API Key"), "sk-x");
    await user.click(screen.getByRole("button", { name: "添加" }));
    expect(await screen.findByText("Base URL 格式无效")).toBeInTheDocument();
    expect(addProvider).not.toHaveBeenCalled();
  });

  it("rejects priority out of range", async () => {
    render(<AddProviderForm />);
    const user = userEvent.setup();
    await user.type(screen.getByLabelText("名称"), "X");
    await user.type(screen.getByLabelText("API Key"), "sk-x");
    const priority = screen.getByLabelText("优先级");
    await user.clear(priority);
    await user.type(priority, "9999");
    await user.click(screen.getByRole("button", { name: "添加" }));
    expect(await screen.findByText("优先级范围为 1--1000")).toBeInTheDocument();
  });

  it("surfaces server errors thrown by addProvider", async () => {
    vi.mocked(addProvider).mockRejectedValueOnce(new Error("boom"));
    render(<AddProviderForm />);
    const user = userEvent.setup();
    await user.type(screen.getByLabelText("名称"), "X");
    await user.type(screen.getByLabelText("API Key"), "sk-x");
    await user.click(screen.getByRole("button", { name: "添加" }));
    expect(await screen.findByText("boom")).toBeInTheDocument();
  });

  it("strips trailing slash from base_url before submitting", async () => {
    vi.mocked(addProvider).mockResolvedValueOnce({
      id: "abc",
      name: "X",
      kind: "openai",
      base_url: "https://api.openai.com",
      priority: 100,
      enabled: true,
      created_at: "",
      updated_at: "",
    });
    render(<AddProviderForm />);
    const user = userEvent.setup();
    await user.type(screen.getByLabelText("名称"), "X");
    const url = screen.getByLabelText("Base URL");
    await user.clear(url);
    await user.type(url, "https://api.openai.com/");
    await user.type(screen.getByLabelText("API Key"), "sk-x");
    await user.click(screen.getByRole("button", { name: "添加" }));
    await waitFor(() => expect(addProvider).toHaveBeenCalledTimes(1));
    expect(vi.mocked(addProvider).mock.calls[0]?.[0].base_url).toBe(
      "https://api.openai.com",
    );
  });

  it("renders structured test results after add", async () => {
    vi.mocked(addProvider).mockResolvedValueOnce({
      id: "abc-123",
      name: "X",
      kind: "anthropic",
      base_url: "https://api.anthropic.com",
      priority: 100,
      enabled: true,
      created_at: "",
      updated_at: "",
    });
    vi.mocked(testProviderConnection).mockResolvedValueOnce({
      status: "failed",
      success: false,
      message: "Not found (404)",
      response_time_ms: 15,
      http_status: 404,
      model_used: "claude-sonnet-4-6",
      tested_at: 1_714_000_000,
      retry_count: 0,
      error_category: "modelNotFound",
    });

    render(<AddProviderForm />);
    const user = userEvent.setup();
    await user.type(screen.getByLabelText("名称"), "X");
    await user.type(screen.getByLabelText("API Key"), "sk-x");
    await user.click(screen.getByRole("button", { name: "添加" }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "测试连接" })).toBeEnabled(),
    );
    await user.click(screen.getByRole("button", { name: "测试连接" }));

    await waitFor(() =>
      expect(testProviderConnection).toHaveBeenCalledWith("abc-123"),
    );
    expect(await screen.findByText(/已添加.*abc-123/)).toBeInTheDocument();
    expect(await screen.findByText(/Not found \(404\)/)).toBeInTheDocument();
  });
});
