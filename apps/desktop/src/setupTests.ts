import "@testing-library/jest-dom";
import { beforeEach, vi } from "vitest";

import i18n from "./i18n";

if (typeof window !== "undefined" && !window.matchMedia) {
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    value: (query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    }),
  });
}

beforeEach(async () => {
  window.localStorage.setItem("i18nextLng", "zh");
  await i18n.changeLanguage("zh");
});
