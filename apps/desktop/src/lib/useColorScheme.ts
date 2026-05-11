import { useEffect, useState } from "react";

type ColorScheme = "light" | "dark";

const QUERY = "(prefers-color-scheme: dark)";

/**
 * 监听系统深浅色偏好，并把结果同步到 `<html>` 的 `.dark` class 与
 * `color-scheme` 上 —— 后者让浏览器原生控件（滚动条 / form 控件）也
 * 跟随主题。返回当前 scheme，供 antd `ConfigProvider` 切换 token。
 */
export function useColorScheme(): ColorScheme {
  const [scheme, setScheme] = useState<ColorScheme>(() =>
    window.matchMedia(QUERY).matches ? "dark" : "light",
  );

  useEffect(() => {
    const media = window.matchMedia(QUERY);
    const handler = (e: MediaQueryListEvent) => {
      setScheme(e.matches ? "dark" : "light");
    };
    media.addEventListener("change", handler);
    return () => media.removeEventListener("change", handler);
  }, []);

  useEffect(() => {
    const root = document.documentElement;
    root.classList.toggle("dark", scheme === "dark");
    root.style.colorScheme = scheme;
  }, [scheme]);

  return scheme;
}
