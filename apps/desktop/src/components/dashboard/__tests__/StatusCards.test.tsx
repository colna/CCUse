import { fireEvent, render, screen } from "@testing-library/react";
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
        kind: "openai",
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
        kind: "openai",
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
          kind: "openai",
          status: "healthy",
          success_rate: 1,
          response_time_us: 20_000,
        },
        {
          provider_id: "provider-1",
          provider_name: "1",
          kind: "openai",
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
          kind: "openai",
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
          kind: "openai",
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

  it("filters providers and refetches metrics when switching to Anthropic", async () => {
    vi.mocked(getHealthSnapshot).mockResolvedValue({
      providers: [
        {
          provider_id: "openai-1",
          provider_name: "OAI",
          kind: "openai",
          status: "healthy",
          success_rate: 1,
          response_time_us: 1_000,
        },
        {
          provider_id: "claude-1",
          provider_name: "Claude",
          kind: "anthropic",
          status: "healthy",
          success_rate: 1,
          response_time_us: 5_000,
        },
      ],
    });

    render(<StatusCards />);

    // Default protocol is OpenAI → current provider should be the openai kind.
    const currentProvider = await screen.findByTestId(
      "current-provider-card-value",
    );
    expect(currentProvider).toHaveTextContent("OAI");
    expect(getMetricsTimeseries).toHaveBeenCalledWith("openai");

    // antd Segmented marks the hidden radios as `pointer-events: none`, so
    // userEvent.click can't reach them. Use fireEvent to bypass the pointer
    // simulation and trigger the change directly — this exercises the same
    // `onChange` path the real UI uses.
    fireEvent.click(screen.getByRole("radio", { name: "Anthropic" }));

    expect(await screen.findByText("Claude")).toBeInTheDocument();
    expect(getMetricsTimeseries).toHaveBeenCalledWith("anthropic");
  });
});
