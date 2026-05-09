import type { Config } from "tailwindcss";
import ccuseTailwindPreset from "@ccuse/ui/tailwind-preset";

const config: Config = {
  presets: [ccuseTailwindPreset],
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        background: "var(--app-bg-layout)",
        foreground: "var(--app-text)",
        card: {
          DEFAULT: "var(--app-bg-container)",
          foreground: "var(--app-text)",
        },
        popover: {
          DEFAULT: "var(--app-bg-elevated)",
          foreground: "var(--app-text)",
        },
        muted: {
          DEFAULT: "var(--app-bg-subtle)",
          foreground: "var(--app-text-secondary)",
        },
        accent: {
          DEFAULT: "var(--app-bg-hover)",
          foreground: "var(--app-text)",
        },
        primary: {
          DEFAULT: "var(--app-primary)",
          foreground: "#ffffff",
        },
        secondary: {
          DEFAULT: "var(--app-bg-hover)",
          foreground: "var(--app-text)",
        },
        destructive: {
          DEFAULT: "var(--app-error)",
          foreground: "#ffffff",
        },
        border: "var(--app-border-secondary)",
        input: "var(--app-border)",
        ring: "var(--app-primary)",
      },
    },
  },
};

export default config;
