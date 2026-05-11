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

/**
 * 包裹全树的 antd `ConfigProvider`：
 * - 主题随系统深浅色自动切换；
 * - antd 内置文案随 i18next 语言切换（目前只支持中英）；
 * - `<App component={false}>` 是 antd 6 的隐藏 message/notification host，
 *   不渲染额外 DOM 节点。
 */
export function AppThemeProvider({ children }: Props) {
  const scheme = useColorScheme();
  const { i18n } = useTranslation();
  const locale = i18n.language?.startsWith("zh") ? zhCN : enUS;

  return (
    <ConfigProvider
      locale={locale}
      theme={scheme === "dark" ? darkTheme : lightTheme}
      button={{ autoInsertSpace: false }}
    >
      <AntApp component={false}>{children}</AntApp>
    </ConfigProvider>
  );
}
