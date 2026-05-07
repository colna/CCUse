import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/tauri", () => ({
  getHealthSnapshot: vi.fn(),
  getMetricsTimeseries: vi.fn(),
  getStrategy: vi.fn(),
  onProviderStatusChanged: vi.fn(),
  refreshHealthSnapshot: vi.fn(),
}));

import {
  getHealthSnapshot,
  getMetricsTimeseries,
  getStrategy,
  onProviderStatusChanged,
  refreshHealthSnapshot,
} from "@/lib/tauri";
import { StatusCards } from "../StatusCards";

beforeEach(() => {
  vi.mocked(getHealthSnapshot).mockReset();
  vi.mocked(getMetricsTimeseries).mockReset();
  vi.mocked(getStrategy).mockReset();
  vi.mocked(onProviderStatusChanged).mockReset();
  vi.mocked(refreshHealthSnapshot).mockReset();
  vi.mocked(getHealthSnapshot).mockResolvedValue({
    providers: [
      {
        provider_id: "claude",
        provider_name: "Claude",
        status: "healthy",
        success_rate: 1,
        response_time_us: 4000,
      },
    ],
  });
  vi.mocked(getMetricsTimeseries).mockResolvedValue([]);
  vi.mocked(getStrategy).mockResolvedValue({
    strategy: "priority",
    max_retries: 3,
    smart_weights: {
      health: 40,
      response_time: 30,
      cost: 20,
      priority: 10,
    },
  });
  vi.mocked(onProviderStatusChanged).mockResolvedValue(() => undefined);
  vi.mocked(refreshHealthSnapshot).mockResolvedValue({
    providers: [
      {
        provider_id: "claude",
        provider_name: "Claude",
        status: "healthy",
        success_rate: 1,
        response_time_us: 4000,
      },
    ],
  });
});

describe("StatusCards", () => {
  it("renders a refresh action", async () => {
    render(<StatusCards />);
    expect(
      await screen.findByRole("button", { name: "刷新" }),
    ).toBeInTheDocument();
  });

  it("refetches when refresh is clicked", async () => {
    render(<StatusCards />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "刷新" }));
    expect(getHealthSnapshot).toHaveBeenCalled();
    expect(refreshHealthSnapshot).toHaveBeenCalled();
    expect(getMetricsTimeseries).toHaveBeenCalled();
  });

  it("shows the lowest-latency provider when fastest strategy is active", async () => {
    vi.mocked(getStrategy).mockResolvedValue({
      strategy: "fastest",
      max_retries: 3,
      smart_weights: {
        health: 40,
        response_time: 30,
        cost: 20,
        priority: 10,
      },
    });
    vi.mocked(getHealthSnapshot).mockResolvedValue({
      providers: [
        {
          provider_id: "provider-2",
          provider_name: "2",
          status: "healthy",
          success_rate: 1,
          response_time_us: 20_000,
        },
        {
          provider_id: "provider-1",
          provider_name: "1",
          status: "healthy",
          success_rate: 1,
          response_time_us: 1_000,
        },
      ],
    });

    render(<StatusCards />);

    expect(
      await screen.findByTestId("current-provider-card-value"),
    ).toHaveTextContent("1");
  });

  it("uses the refreshed health snapshot when refresh is clicked", async () => {
    vi.mocked(getStrategy).mockResolvedValue({
      strategy: "fastest",
      max_retries: 3,
      smart_weights: {
        health: 40,
        response_time: 30,
        cost: 20,
        priority: 10,
      },
    });
    vi.mocked(getHealthSnapshot).mockResolvedValue({
      providers: [
        {
          provider_id: "provider-2",
          provider_name: "2",
          status: "healthy",
          success_rate: 1,
          response_time_us: 2_000,
        },
      ],
    });
    vi.mocked(refreshHealthSnapshot).mockResolvedValue({
      providers: [
        {
          provider_id: "provider-1",
          provider_name: "1",
          status: "healthy",
          success_rate: 1,
          response_time_us: 1_000,
        },
      ],
    });

    render(<StatusCards />);

    const currentProvider = await screen.findByTestId(
      "current-provider-card-value",
    );
    expect(currentProvider).toHaveTextContent("2");

    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "刷新" }));

    expect(refreshHealthSnapshot).toHaveBeenCalledTimes(1);
    expect(currentProvider).toHaveTextContent("1");
  });
});
