import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/tauri", () => ({
  getStrategy: vi.fn(),
  updateStrategyParams: vi.fn(),
}));

import { getStrategy, updateStrategyParams } from "@/lib/tauri";
import { AdvancedParams } from "../AdvancedParams";

function slider(name: string): HTMLInputElement {
  return screen.getByRole("slider", { name }) as HTMLInputElement;
}

describe("AdvancedParams", () => {
  beforeEach(() => {
    vi.mocked(getStrategy).mockReset();
    vi.mocked(updateStrategyParams).mockReset();
    vi.mocked(getStrategy).mockResolvedValue({
      strategy: "smart",
      max_retries: 3,
      smart_weights: {
        health: 40,
        response_time: 30,
        cost: 20,
        priority: 10,
      },
    });
    vi.mocked(updateStrategyParams).mockResolvedValue(undefined);
  });

  it("scales the other smart weights proportionally and keeps total at 100", async () => {
    render(<AdvancedParams />);

    await screen.findByText("智能策略权重");
    fireEvent.change(slider("健康度"), { target: { value: "50" } });

    expect(slider("健康度").value).toBe("50");
    expect(slider("响应速度").value).toBe("25");
    expect(slider("成本").value).toBe("17");
    expect(slider("优先级").value).toBe("8");
    expect(screen.getByText("总和：100 / 100")).toBeInTheDocument();
    expect(screen.getByText("已校验：权重总和为 100")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "保存" }));

    await waitFor(() => expect(updateStrategyParams).toHaveBeenCalledTimes(1));
    expect(updateStrategyParams).toHaveBeenCalledWith({
      max_retries: 3,
      smart_weights: {
        health: 50,
        response_time: 25,
        cost: 17,
        priority: 8,
      },
    });
  });

  it("distributes remaining weight evenly when the other sliders are zero", async () => {
    vi.mocked(getStrategy).mockResolvedValueOnce({
      strategy: "smart",
      max_retries: 3,
      smart_weights: {
        health: 100,
        response_time: 0,
        cost: 0,
        priority: 0,
      },
    });

    render(<AdvancedParams />);

    await screen.findByText("智能策略权重");
    fireEvent.change(slider("健康度"), { target: { value: "25" } });

    expect(slider("健康度").value).toBe("25");
    expect(slider("响应速度").value).toBe("25");
    expect(slider("成本").value).toBe("25");
    expect(slider("优先级").value).toBe("25");
    expect(screen.getByText("总和：100 / 100")).toBeInTheDocument();
  });
});
