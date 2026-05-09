import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Button } from "../button";

describe("Button", () => {
  it("renders as a native <button> by default", () => {
    render(<Button>Click me</Button>);
    const btn = screen.getByRole("button", { name: "Click me" });
    expect(btn.tagName).toBe("BUTTON");
  });

  it("applies antd primary type when type=primary", () => {
    render(<Button type="primary">Primary</Button>);
    const btn = screen.getByRole("button", { name: "Primary" });
    expect(btn.className).toMatch(/ant-btn(-color)?-primary/);
  });

  it("applies antd default type by default", () => {
    render(<Button>Default</Button>);
    const btn = screen.getByRole("button", { name: "Default" });
    expect(btn.className).toContain("ant-btn");
  });

  it("supports size attribute via antd size prop", () => {
    render(<Button size="large">Large</Button>);
    const btn = screen.getByRole("button", { name: "Large" });
    expect(btn.className).toContain("ant-btn-lg");
  });

  it("forwards extra className", () => {
    render(<Button className="my-custom">Spaced</Button>);
    const btn = screen.getByRole("button", { name: "Spaced" });
    expect(btn.className).toContain("my-custom");
  });

  it("invokes onClick when activated", async () => {
    const onClick = vi.fn();
    render(<Button onClick={onClick}>Go</Button>);
    await userEvent.click(screen.getByRole("button", { name: "Go" }));
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it("respects disabled attribute", () => {
    render(<Button disabled>Off</Button>);
    expect(screen.getByRole("button", { name: "Off" })).toBeDisabled();
  });

  it("does not auto insert space between Chinese characters", () => {
    render(<Button>添加</Button>);
    const btn = screen.getByRole("button", { name: "添加" });
    expect(btn).toBeInTheDocument();
  });
});
