import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import LanguageDetector from "i18next-browser-languagedetector";

import enCommon from "./locales/en/common.json";
import enProviders from "./locales/en/providers.json";
import enStrategy from "./locales/en/strategy.json";
import enMonitor from "./locales/en/monitor.json";
import zhCommon from "./locales/zh/common.json";
import zhProviders from "./locales/zh/providers.json";
import zhStrategy from "./locales/zh/strategy.json";
import zhMonitor from "./locales/zh/monitor.json";

/**
 * i18next 命名空间按页面拆：常用文案在 `common`，业务页面各自一个
 * namespace。`defaultNS` 设为 common 以便 `useTranslation()` 无参数
 * 时直接拿到通用文案。
 *
 * 语言检测顺序：先看用户上次显式选择（localStorage），再回退到浏览器
 * 语言；`Settings` 页里"跟随系统"选项会主动清掉 localStorage，回到
 * 检测链。
 */

const NAMESPACES = ["common", "providers", "strategy", "monitor"] as const;

const resources = {
  en: {
    common: enCommon,
    providers: enProviders,
    strategy: enStrategy,
    monitor: enMonitor,
  },
  zh: {
    common: zhCommon,
    providers: zhProviders,
    strategy: zhStrategy,
    monitor: zhMonitor,
  },
};

i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources,
    fallbackLng: "en",
    ns: NAMESPACES as unknown as string[],
    defaultNS: "common",
    interpolation: { escapeValue: false },
    detection: {
      order: ["localStorage", "navigator"],
      caches: ["localStorage"],
      lookupLocalStorage: "i18nextLng",
    },
  });

export default i18n;
