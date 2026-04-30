import type { Config } from "tailwindcss";
import ccuseTailwindPreset from "@ccuse/ui/tailwind-preset";

const config: Config = {
  presets: [ccuseTailwindPreset],
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
};

export default config;
