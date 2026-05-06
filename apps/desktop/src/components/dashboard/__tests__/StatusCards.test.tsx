import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/tauri", () => ({
  getHealthSnapshot: vi.fn(),
  getMetricsTimeseries: vi.fn(),
}));

import { getHealthSnapshot, getMetricsTimeseries } from "@/lib/tauri";
import { StatusCards } from "../StatusCards";

beforeEach(() => {
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
    expect(getMetricsTimeseries).toHaveBeenCalled();
  });
});
