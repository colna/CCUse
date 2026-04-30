import { getRequestConfig } from "next-intl/server";

import { defaultLocale, isLocale } from "./routing";

const messageLoaders = {
  zh: () => import("../messages/zh.json"),
  en: () => import("../messages/en.json"),
};

export default getRequestConfig(async ({ requestLocale }) => {
  const requestedLocale = await requestLocale;
  const locale = isLocale(requestedLocale) ? requestedLocale : defaultLocale;
  const messages = (await messageLoaders[locale]()).default;

  return {
    locale,
    messages,
  };
});
