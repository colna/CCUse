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
    ns: ["common", "providers", "strategy", "monitor"],
    defaultNS: "common",
    interpolation: {
      escapeValue: false,
    },
    detection: {
      order: ["localStorage", "navigator"],
      caches: ["localStorage"],
      lookupLocalStorage: "i18nextLng",
    },
  });

export default i18n;
