import { App as AntApp, ConfigProvider } from "antd";
import enUS from "antd/locale/en_US";
import zhCN from "antd/locale/zh_CN";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { darkTheme, lightTheme } from "@/lib/theme";
import { useColorScheme } from "@/lib/useColorScheme";

interface Props {
  children: ReactNode;
}

export function AppThemeProvider({ children }: Props) {
  const scheme = useColorScheme();
  const { i18n } = useTranslation();
  const locale = i18n.language?.startsWith("zh") ? zhCN : enUS;

  return (
    <ConfigProvider
      locale={locale}
      theme={scheme === "dark" ? darkTheme : lightTheme}
    >
      <AntApp component={false}>{children}</AntApp>
    </ConfigProvider>
  );
}
