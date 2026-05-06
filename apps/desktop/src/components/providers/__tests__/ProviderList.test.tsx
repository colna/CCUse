import { render, screen, waitFor } from "@testing-library/react";
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
  vi.mocked(testProviderConnection).mockResolvedValue(88);
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

  it("edits the provider type and saves it", async () => {
    render(<ProviderList />);
    const user = userEvent.setup();

    await user.click(
      await screen.findByRole("button", { name: /编辑 Claude Prod/ }),
    );
    await user.selectOptions(screen.getByLabelText("供应商类型"), "openai");
    await user.click(screen.getByRole("button", { name: "保存修改" }));

    await waitFor(() =>
      expect(updateProvider).toHaveBeenCalledWith("provider-1", {
        name: "Claude Prod",
        kind: "openai",
        base_url: "https://api.anthropic.com",
        api_key: "",
        priority: 10,
        enabled: true,
      }),
    );
  });
});
