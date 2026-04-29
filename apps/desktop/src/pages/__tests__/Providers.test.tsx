import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/tauri", () => ({
  addProvider: vi.fn(),
  deleteProvider: vi.fn(),
  getHealthSnapshot: vi.fn(),
  listProviders: vi.fn(),
  testProviderConnection: vi.fn(),
  updateProvider: vi.fn(),
}));

import {
  addProvider,
  getHealthSnapshot,
  listProviders,
  type Provider,
} from "@/lib/tauri";
import { ProvidersPage } from "../Providers";

const ADDED_PROVIDER: Provider = {
  id: "provider-hot-reload",
  name: "Work OpenAI",
  kind: "openai",
  base_url: "https://api.openai.com",
  priority: 10,
  enabled: true,
  monthly_quota: null,
  rate_limit_rpm: null,
  cost_per_1k_tokens: null,
  created_at: "2026-04-29T00:00:00.000Z",
  updated_at: "2026-04-29T00:00:00.000Z",
};

let providers: Provider[];

describe("ProvidersPage", () => {
  beforeEach(() => {
    providers = [];
    vi.mocked(listProviders).mockReset();
    vi.mocked(listProviders).mockImplementation(async () => providers);
    vi.mocked(addProvider).mockReset();
    vi.mocked(addProvider).mockImplementation(async () => {
      providers = [ADDED_PROVIDER];
      return ADDED_PROVIDER;
    });
    vi.mocked(getHealthSnapshot).mockReset();
    vi.mocked(getHealthSnapshot).mockResolvedValue({ providers: [] });
  });

  it("refreshes the provider list after adding a provider", async () => {
    render(<ProvidersPage />);
    const user = userEvent.setup();

    expect(
      await screen.findByText("暂无供应商，请在下方添加。"),
    ).toBeInTheDocument();

    await user.type(screen.getByLabelText("名称"), "Work OpenAI");
    await user.clear(screen.getByLabelText("优先级"));
    await user.type(screen.getByLabelText("优先级"), "10");
    await user.type(screen.getByLabelText("API Key"), "sk-test");
    await user.click(screen.getByRole("button", { name: "添加" }));

    await waitFor(() => expect(addProvider).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(listProviders).toHaveBeenCalledTimes(2));
    expect(await screen.findByText("Work OpenAI")).toBeInTheDocument();
    expect(screen.getByText("openai · 优先级 10")).toBeInTheDocument();
  });
});
