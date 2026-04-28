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

  it("applies primary variant classes by default", () => {
    render(<Button>Primary</Button>);
    const btn = screen.getByRole("button", { name: "Primary" });
    expect(btn.className).toContain("bg-primary");
    expect(btn.className).toContain("text-primary-foreground");
  });

  it("applies pill variant classes when variant=pill", () => {
    render(<Button variant="pill">Learn more</Button>);
    const btn = screen.getByRole("button", { name: "Learn more" });
    expect(btn.className).toContain("rounded-full");
    expect(btn.className).toContain("border");
  });

  it("applies size classes", () => {
    render(<Button size="lg">Large</Button>);
    expect(screen.getByRole("button").className).toContain("h-10");
  });

  it("forwards extra className via tailwind-merge without conflict", () => {
    render(<Button className="px-12">Spaced</Button>);
    const btn = screen.getByRole("button", { name: "Spaced" });
    expect(btn.className).toContain("px-12");
    // Conflicting size px-4 from default size should be replaced
    expect(btn.className).not.toMatch(/(?:^|\s)px-4(?:\s|$)/);
  });

  it("forwards ref to the underlying button element", () => {
    const ref = { current: null as HTMLButtonElement | null };
    render(<Button ref={ref}>Ref</Button>);
    expect(ref.current?.tagName).toBe("BUTTON");
  });

  it("renders as child element when asChild is true (Slot)", () => {
    render(
      <Button asChild>
        <a href="/docs">Docs</a>
      </Button>,
    );
    const link = screen.getByRole("link", { name: "Docs" });
    expect(link.tagName).toBe("A");
    expect(link.getAttribute("href")).toBe("/docs");
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
});
