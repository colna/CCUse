import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import App from "../App";

describe("App", () => {
  it("renders product title heading", () => {
    render(<App />);
    expect(
      screen.getByRole("heading", { name: "CCUse" }),
    ).toBeInTheDocument();
  });

  it("renders product tagline copy", () => {
    render(<App />);
    expect(
      screen.getByText(/本地 API 代理.*多供应商无感切换/),
    ).toBeInTheDocument();
  });

  it("shows the version + phase indicator", () => {
    render(<App />);
    expect(screen.getByText(/v0\.0\.0/)).toBeInTheDocument();
    expect(screen.getByText(/Phase 1\.0\.1/)).toBeInTheDocument();
  });

  it("renders the two CTA buttons (default + pill)", () => {
    render(<App />);
    expect(
      screen.getByRole("button", { name: "开始配置" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "查看文档" }),
    ).toBeInTheDocument();
  });
});
