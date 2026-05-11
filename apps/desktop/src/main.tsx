import React from "react";
import ReactDOM from "react-dom/client";

import App from "./App";
import { AppThemeProvider } from "./components/providers/AppThemeProvider";
import "./i18n";
import "./globals.css";

// `index.html` 里的根节点是固定 id="root"；缺失即视为构建出错，直接抛
// 错让我们在开发期能立刻看到原因，而不是悄无声息空白。
const root = document.getElementById("root");
if (!root) throw new Error("Root element #root not found in index.html");

ReactDOM.createRoot(root).render(
  <React.StrictMode>
    <AppThemeProvider>
      <App />
    </AppThemeProvider>
  </React.StrictMode>,
);
