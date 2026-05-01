import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect } from "vitest";

import App from "../App";

describe("App shell", () => {
  it("redirects from / to /dashboard and shows the dashboard heading", async () => {
    render(<App />);
    expect(
      await screen.findByRole("heading", { level: 2, name: "总览" }),
    ).toBeInTheDocument();
  });

  it("renders the topbar version + phase indicator", () => {
    render(<App />);
    expect(screen.getByText(/v0\.0\.0/)).toBeInTheDocument();
    expect(screen.getByText(/Phase 1\.0\.1/)).toBeInTheDocument();
  });

  it("exposes the three primary nav items", () => {
    render(<App />);
    const nav = screen.getByRole("navigation", { name: "主导航" });
    for (const label of ["总览", "供应商", "设置"]) {
      expect(within(nav, label)).toBeInTheDocument();
    }
  });

  it("navigates to providers when the providers nav link is clicked", async () => {
    render(<App />);
    const user = userEvent.setup();
    const nav = screen.getByRole("navigation", { name: "主导航" });
    const providersLink = within(nav, "供应商");
    await user.click(providersLink);
    expect(
      await screen.findByRole("heading", { level: 2, name: "供应商" }),
    ).toBeInTheDocument();
  });

  it("navigates to settings when the settings nav link is clicked", async () => {
    render(<App />);
    const user = userEvent.setup();
    const nav = screen.getByRole("navigation", { name: "主导航" });
    const settingsLink = within(nav, "设置");
    await user.click(settingsLink);
    expect(
      await screen.findByRole("heading", { level: 2, name: "设置" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "官网" })).toHaveAttribute(
      "href",
      "https://ccuse.app",
    );
    expect(screen.getByRole("link", { name: "下载页" })).toHaveAttribute(
      "href",
      "https://ccuse.app/download",
    );
  });
});

/** Find a child of `container` whose accessible name matches `text`.
 * Helps disambiguate between sidebar links and main-section headings
 * that carry the same Chinese label. */
function within(container: HTMLElement, text: string) {
  const link = container.querySelector(`a[aria-current], a, button`);
  if (link) {
    const matches = Array.from(
      container.querySelectorAll<HTMLElement>("a, button"),
    ).filter((el) => el.textContent?.trim() === text);
    if (matches.length === 1) return matches[0]!;
  }
  throw new Error(`No nav item with text "${text}" found in container`);
}
