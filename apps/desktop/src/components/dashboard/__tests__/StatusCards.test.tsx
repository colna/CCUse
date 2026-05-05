import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/tauri", () => ({
  getCurrentProvider: vi.fn(),
  getHealthSnapshot: vi.fn(),
  getMetricsTimeseries: vi.fn(),
  refreshHealthSnapshot: vi.fn(),
}));

import {
  getCurrentProvider,
  getHealthSnapshot,
  getMetricsTimeseries,
  refreshHealthSnapshot,
} from "@/lib/tauri";
import { StatusCards } from "../StatusCards";

describe("StatusCards", () => {
  beforeEach(() => {
    vi.mocked(getCurrentProvider).mockReset();
    vi.mocked(getCurrentProvider).mockResolvedValue({
      provider_id: null,
      provider_name: null,
      model: null,
      status: null,
      last_request_at: null,
    });
    vi.mocked(getHealthSnapshot).mockReset();
    vi.mocked(getHealthSnapshot).mockResolvedValue({
      providers: [
        {
          provider_id: "cached",
          provider_name: "Cached Provider",
          status: "healthy",
          success_rate: 1,
          response_time_us: 10_000,
        },
      ],
    });
    vi.mocked(getMetricsTimeseries).mockReset();
    vi.mocked(getMetricsTimeseries).mockResolvedValue([]);
    vi.mocked(refreshHealthSnapshot).mockReset();
    vi.mocked(refreshHealthSnapshot).mockResolvedValue({
      providers: [
        {
          provider_id: "live",
          provider_name: "Live Provider",
          status: "healthy",
          success_rate: 1,
          response_time_us: 12_000,
        },
      ],
    });
  });

  it("refreshes current available provider information on demand", async () => {
    render(<StatusCards />);
    const user = userEvent.setup();

    expect(await screen.findByText("Cached Provider")).toBeInTheDocument();

    await user.click(
      screen.getByRole("button", {
        name: "刷新当前可用供应商信息",
      }),
    );

    await waitFor(() => expect(refreshHealthSnapshot).toHaveBeenCalledTimes(1));
    expect(await screen.findByText("Live Provider")).toBeInTheDocument();
  });
});
