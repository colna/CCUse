import "@testing-library/jest-dom";
import { beforeEach } from "vitest";

import i18n from "./i18n";

beforeEach(async () => {
  window.localStorage.setItem("i18nextLng", "zh");
  await i18n.changeLanguage("zh");
});
