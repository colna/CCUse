import "@testing-library/jest-dom";
import { beforeEach, vi } from "vitest";

import i18n from "./i18n";

// jsdom 不实现 matchMedia；很多组件（含 useColorScheme）会在 mount
// 时调它，缺失会抛 TypeError。这里给一个永远返回"亮色"的桩函数。
if (!window.matchMedia) {
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    value: (query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      addListener: vi.fn(),
      removeListener: vi.fn(),
      dispatchEvent: vi.fn(),
    }),
  });
}

// 测试断言里大量写死中文文案；每个用例都从中文起步以避免 i18n
// 偶然漂移到 en。
beforeEach(async () => {
  window.localStorage.setItem("i18nextLng", "zh");
  await i18n.changeLanguage("zh");
});
