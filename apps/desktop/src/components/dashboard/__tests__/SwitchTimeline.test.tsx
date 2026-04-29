import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/tauri", () => ({
  getSwitchTimeline: vi.fn(),
}));

import { getSwitchTimeline, type SwitchEvent } from "@/lib/tauri";
import { SwitchTimeline } from "../SwitchTimeline";

const EVENT: SwitchEvent = {
  id: "1",
  timestamp: "2026-04-29T12:00:00.000Z",
  from_provider: "primary-503",
  to_provider: "backup-after-503",
  strategy: "priority",
  reason: "upstream_503",
};

beforeEach(() => {
  vi.mocked(getSwitchTimeline).mockReset();
});

describe("SwitchTimeline", () => {
  it("renders a failover event and expands its reason", async () => {
    vi.mocked(getSwitchTimeline).mockResolvedValue([EVENT]);

    render(<SwitchTimeline />);

    const row = await screen.findByRole("button", {
      name: /primary-503.*backup-after-503.*priority/,
    });
    expect(row).toBeInTheDocument();

    const user = userEvent.setup();
    await user.click(row);

    await waitFor(() => expect(getSwitchTimeline).toHaveBeenCalledTimes(1));
    expect(screen.getByText("upstream_503")).toBeInTheDocument();
    expect(screen.getByText("1")).toBeInTheDocument();
  });

  it("renders the empty state when no switch events exist", async () => {
    vi.mocked(getSwitchTimeline).mockResolvedValue([]);

    render(<SwitchTimeline />);

    expect(await screen.findByText("暂无切换事件")).toBeInTheDocument();
  });
});
