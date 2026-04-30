import ccuseTailwindPreset from "@ccuse/ui/tailwind-preset";
import type { Config } from "tailwindcss";

const config: Config = {
  presets: [ccuseTailwindPreset],
  content: [
    "./app/**/*.{ts,tsx}",
    "./components/**/*.{ts,tsx}",
    "../../packages/ui/src/**/*.{ts,tsx}",
    "../../packages/ui/tailwind-preset.js",
  ],
};

export default config;
