import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ConfigExportPanel } from "../ConfigExportPanel";

vi.mock("@/lib/tauri", () => ({
  exportConfig: vi.fn().mockResolvedValue(new Uint8Array([1, 2, 3])),
  importConfig: vi.fn().mockResolvedValue(undefined),
  getTemplatePresets: vi.fn().mockResolvedValue([
    {
      id: "openai",
      name: "OpenAI",
      description: "OpenAI GPT models",
      providers: [
        {
          name: "OpenAI",
          kind: "openai",
          base_url: "https://api.openai.com",
          priority: 10,
          enabled: true,
        },
      ],
    },
    {
      id: "claude",
      name: "Claude (Anthropic)",
      description: "Anthropic Claude",
      providers: [],
    },
    {
      id: "gemini",
      name: "Google Gemini",
      description: "Gemini API",
      providers: [],
    },
  ]),
}));

describe("ConfigExportPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders export and import buttons", () => {
    render(<ConfigExportPanel />);
    expect(
      screen.getByRole("button", { name: /Export Config/i }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /Import Config/i }),
    ).toBeInTheDocument();
  });

  it("renders heading and description text", () => {
    render(<ConfigExportPanel />);
    expect(screen.getByText("Config Export / Import")).toBeInTheDocument();
    expect(
      screen.getByText(/Export your provider configuration/i),
    ).toBeInTheDocument();
  });

  it("loads and displays template presets", async () => {
    render(<ConfigExportPanel />);
    await waitFor(() => {
      expect(screen.getByText("OpenAI")).toBeInTheDocument();
    });
    expect(screen.getByText("Claude (Anthropic)")).toBeInTheDocument();
    expect(screen.getByText("Google Gemini")).toBeInTheDocument();
  });

  it("shows status message when preset is clicked", async () => {
    const user = userEvent.setup();
    render(<ConfigExportPanel />);
    await waitFor(() => {
      expect(screen.getByText("OpenAI")).toBeInTheDocument();
    });
    await user.click(screen.getByText("OpenAI"));
    expect(screen.getByText(/Template "OpenAI" selected/i)).toBeInTheDocument();
  });

  it("renders Quick-Start Templates heading", async () => {
    render(<ConfigExportPanel />);
    await waitFor(() => {
      expect(screen.getByText("Quick-Start Templates")).toBeInTheDocument();
    });
  });
});
