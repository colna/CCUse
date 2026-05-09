import { theme as antdTheme, type ThemeConfig } from "antd";

const APPLE_FONT_STACK = [
  "-apple-system",
  "BlinkMacSystemFont",
  '"SF Pro Text"',
  '"Segoe UI Variable Display"',
  '"Segoe UI"',
  "system-ui",
  "sans-serif",
].join(",");

const SHARED_TOKEN = {
  colorPrimary: "#0071e3",
  colorInfo: "#0071e3",
  colorLink: "#0066cc",
  borderRadius: 8,
  borderRadiusLG: 12,
  borderRadiusSM: 6,
  controlHeight: 32,
  controlHeightSM: 28,
  controlHeightLG: 40,
  fontFamily: APPLE_FONT_STACK,
  fontSize: 14,
  wireframe: false,
  motionDurationMid: "0.18s",
  motionDurationFast: "0.12s",
} as const;

export const lightTheme: ThemeConfig = {
  algorithm: antdTheme.defaultAlgorithm,
  token: {
    ...SHARED_TOKEN,
    colorBgLayout: "#f5f5f7",
    colorBgContainer: "#ffffff",
    colorBgElevated: "#ffffff",
    colorText: "#1d1d1f",
    colorTextSecondary: "rgba(0, 0, 0, 0.65)",
    colorTextTertiary: "rgba(0, 0, 0, 0.48)",
    colorBorder: "rgba(0, 0, 0, 0.08)",
    colorBorderSecondary: "rgba(0, 0, 0, 0.06)",
    boxShadow: "0 1px 2px rgba(0, 0, 0, 0.04), 0 4px 12px rgba(0, 0, 0, 0.06)",
    boxShadowSecondary:
      "0 1px 2px rgba(0, 0, 0, 0.03), 0 2px 6px rgba(0, 0, 0, 0.04)",
  },
  components: {
    Button: {
      primaryShadow: "none",
      defaultShadow: "none",
      dangerShadow: "none",
      controlHeight: 32,
      paddingInline: 14,
    },
    Card: {
      headerBg: "transparent",
    },
    Layout: {
      headerBg: "rgba(255, 255, 255, 0.72)",
      siderBg: "rgba(255, 255, 255, 0.6)",
      bodyBg: "#f5f5f7",
    },
    Menu: {
      itemBg: "transparent",
      itemSelectedBg: "rgba(0, 113, 227, 0.08)",
      itemSelectedColor: "#0071e3",
      itemHoverBg: "rgba(0, 0, 0, 0.04)",
      itemBorderRadius: 8,
      itemMarginInline: 8,
    },
  },
};

export const darkTheme: ThemeConfig = {
  algorithm: antdTheme.darkAlgorithm,
  token: {
    ...SHARED_TOKEN,
    colorBgLayout: "#0a0a0a",
    colorBgContainer: "#1c1c1e",
    colorBgElevated: "#28282a",
    colorText: "#f5f5f7",
    colorTextSecondary: "rgba(255, 255, 255, 0.65)",
    colorTextTertiary: "rgba(255, 255, 255, 0.48)",
    colorBorder: "rgba(255, 255, 255, 0.08)",
    colorBorderSecondary: "rgba(255, 255, 255, 0.06)",
    boxShadow: "0 1px 2px rgba(0, 0, 0, 0.4), 0 4px 16px rgba(0, 0, 0, 0.5)",
    boxShadowSecondary:
      "0 1px 2px rgba(0, 0, 0, 0.3), 0 2px 6px rgba(0, 0, 0, 0.4)",
  },
  components: {
    Button: {
      primaryShadow: "none",
      defaultShadow: "none",
      dangerShadow: "none",
      controlHeight: 32,
      paddingInline: 14,
    },
    Card: {
      headerBg: "transparent",
    },
    Layout: {
      headerBg: "rgba(28, 28, 30, 0.72)",
      siderBg: "rgba(28, 28, 30, 0.6)",
      bodyBg: "#0a0a0a",
    },
    Menu: {
      itemBg: "transparent",
      itemSelectedBg: "rgba(41, 151, 255, 0.16)",
      itemSelectedColor: "#2997ff",
      itemHoverBg: "rgba(255, 255, 255, 0.06)",
      itemBorderRadius: 8,
      itemMarginInline: 8,
    },
  },
};
