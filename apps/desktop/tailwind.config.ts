import type { Config } from "tailwindcss";
import ccuseTailwindPreset from "@ccuse/ui/tailwind-preset";

const config: Config = {
  presets: [ccuseTailwindPreset],
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        background: "var(--ant-color-bg-layout)",
        foreground: "var(--ant-color-text)",
        card: {
          DEFAULT: "var(--ant-color-bg-container)",
          foreground: "var(--ant-color-text)",
        },
        popover: {
          DEFAULT: "var(--ant-color-bg-elevated)",
          foreground: "var(--ant-color-text)",
        },
        muted: {
          DEFAULT: "var(--ant-color-fill-quaternary)",
          foreground: "var(--ant-color-text-secondary)",
        },
        accent: {
          DEFAULT: "var(--ant-color-fill-tertiary)",
          foreground: "var(--ant-color-text)",
        },
        primary: {
          DEFAULT: "var(--ant-color-primary)",
          foreground: "var(--ant-color-white, #ffffff)",
        },
        secondary: {
          DEFAULT: "var(--ant-color-fill-secondary)",
          foreground: "var(--ant-color-text)",
        },
        destructive: {
          DEFAULT: "var(--ant-color-error)",
          foreground: "var(--ant-color-white, #ffffff)",
        },
        border: "var(--ant-color-border-secondary)",
        input: "var(--ant-color-border)",
        ring: "var(--ant-color-primary)",
      },
    },
  },
};

export default config;
