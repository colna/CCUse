import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/tauri", () => ({
  deleteProvider: vi.fn(),
  getHealthSnapshot: vi.fn(),
  listProviders: vi.fn(),
  onProviderStatusChanged: vi.fn(),
  testProviderConnection: vi.fn(),
  updateProvider: vi.fn(),
}));

import {
  deleteProvider,
  getHealthSnapshot,
  listProviders,
  onProviderStatusChanged,
  testProviderConnection,
  updateProvider,
} from "@/lib/tauri";
import { ProviderList } from "../ProviderList";

const provider = {
  id: "provider-1",
  name: "Claude Prod",
  kind: "anthropic" as const,
  base_url: "https://api.anthropic.com",
  priority: 10,
  enabled: true,
  created_at: "2026-04-29T00:00:00.000Z",
  updated_at: "2026-04-29T00:00:00.000Z",
};

beforeEach(() => {
  vi.mocked(listProviders).mockResolvedValue([provider]);
  vi.mocked(getHealthSnapshot).mockResolvedValue({
    providers: [
      {
        provider_id: "provider-1",
        provider_name: "Claude Prod",
        status: "down",
        success_rate: 0,
        response_time_us: null,
      },
    ],
  });
  vi.mocked(onProviderStatusChanged).mockResolvedValue(() => undefined);
  vi.mocked(testProviderConnection).mockResolvedValue({
    status: "operational",
    success: true,
    message: "Check succeeded",
    response_time_ms: 88,
    http_status: 200,
    model_used: "gpt-5.5-instant",
    tested_at: 1_714_000_000,
    retry_count: 0,
    error_category: null,
  });
  vi.mocked(updateProvider).mockResolvedValue({
    ...provider,
    kind: "openai",
  });
});

describe("ProviderList", () => {
  it("shows a per-provider health test button", async () => {
    render(<ProviderList />);
    expect(
      await screen.findByRole("button", {
        name: /测试 Claude Prod 的健康状态/,
      }),
    ).toBeInTheDocument();
  });

  it("runs the provider health test when clicked", async () => {
    render(<ProviderList />);
    const user = userEvent.setup();
    await user.click(
      await screen.findByRole("button", {
        name: /测试 Claude Prod 的健康状态/,
      }),
    );
    await waitFor(() =>
      expect(testProviderConnection).toHaveBeenCalledWith("provider-1"),
    );
    expect(deleteProvider).not.toHaveBeenCalled();
  });

  it("shows structured model test failure details", async () => {
    vi.mocked(testProviderConnection).mockResolvedValueOnce({
      status: "failed",
      success: false,
      message: "Not found (404)",
      response_time_ms: 12,
      http_status: 404,
      model_used: "claude-sonnet-4-6",
      tested_at: 1_714_000_000,
      retry_count: 0,
      error_category: "modelNotFound",
    });

    render(<ProviderList />);
    const user = userEvent.setup();
    await user.click(
      await screen.findByRole("button", {
        name: /测试 Claude Prod 的健康状态/,
      }),
    );

    await waitFor(() =>
      expect(testProviderConnection).toHaveBeenCalledWith("provider-1"),
    );
    expect(await screen.findByText("失败")).toBeInTheDocument();
    expect(await screen.findByText(/modelNotFound/)).toBeInTheDocument();
  });

  it("keeps the provider list visible and shows a dialog when health test fails", async () => {
    vi.mocked(testProviderConnection).mockRejectedValueOnce(
      new Error("HTTP 401 invalid key"),
    );
    render(<ProviderList />);
    const user = userEvent.setup();

    await user.click(
      await screen.findByRole("button", {
        name: /测试 Claude Prod 的健康状态/,
      }),
    );

    const dialog = await screen.findByRole("alertdialog");
    expect(dialog).toBeInTheDocument();
    expect(within(dialog).getByText("连接测试失败")).toBeInTheDocument();
    expect(
      within(dialog).getByText("HTTP 401 invalid key"),
    ).toBeInTheDocument();
    expect(within(dialog).getByText("Claude Prod")).toBeInTheDocument();
    expect(
      screen.getByRole("button", {
        name: /测试 Claude Prod 的健康状态/,
      }),
    ).toBeInTheDocument();
  });

  it("edits the provider type and saves it", async () => {
    render(<ProviderList />);
    const user = userEvent.setup();

    await user.click(
      await screen.findByRole("button", { name: /编辑 Claude Prod/ }),
    );
    await user.selectOptions(screen.getByLabelText("供应商类型"), "openai");
    expect(screen.getByRole("option", { name: "Gemini" })).toBeDisabled();
    expect(screen.getByRole("option", { name: "Relay" })).toBeDisabled();
    expect(screen.getByRole("option", { name: "Custom" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "保存修改" }));

    await waitFor(() =>
      expect(updateProvider).toHaveBeenCalledWith("provider-1", {
        name: "Claude Prod",
        kind: "openai",
        base_url: "https://api.anthropic.com",
        api_key: "",
        priority: 10,
        enabled: true,
        monthly_quota: null,
        rate_limit_rpm: null,
        cost_per_1k_tokens: null,
      }),
    );
  });

  it("submits a replacement api key when editing the provider", async () => {
    render(<ProviderList />);
    const user = userEvent.setup();

    await user.click(
      await screen.findByRole("button", { name: /编辑 Claude Prod/ }),
    );
    await user.type(screen.getByLabelText("API Key"), "sk-new-secret");
    await user.click(screen.getByRole("button", { name: "保存修改" }));

    await waitFor(() =>
      expect(updateProvider).toHaveBeenCalledWith("provider-1", {
        name: "Claude Prod",
        kind: "anthropic",
        base_url: "https://api.anthropic.com",
        api_key: "sk-new-secret",
        priority: 10,
        enabled: true,
        monthly_quota: null,
        rate_limit_rpm: null,
        cost_per_1k_tokens: null,
      }),
    );
  });
});
