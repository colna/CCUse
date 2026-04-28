import { invoke } from "@tauri-apps/api/core";

/** Wire shape returned by the `get_local_api_config` Tauri command.
 * Mirrors `proxy::runtime::LocalApiConfig` in the Rust backend. */
export interface LocalApiConfig {
  base_url: string;
  api_key: string;
}

export async function getLocalApiConfig(): Promise<LocalApiConfig> {
  return invoke<LocalApiConfig>("get_local_api_config");
}

export async function regenerateApiKey(): Promise<LocalApiConfig> {
  return invoke<LocalApiConfig>("regenerate_api_key");
}

export async function restartProxy(): Promise<LocalApiConfig> {
  return invoke<LocalApiConfig>("restart_proxy");
}

/** Copy text to the system clipboard.
 *
 * Browser Clipboard API is the happy path; we keep the
 * `document.execCommand` fallback because Tauri's webview on
 * older WebKit / WebView2 builds occasionally exposes neither. */
export async function copyToClipboard(text: string): Promise<void> {
  if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    return;
  }
  if (typeof document === "undefined") return;
  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.setAttribute("readonly", "");
  textarea.style.position = "fixed";
  document.body.appendChild(textarea);
  textarea.select();
  document.execCommand("copy");
  document.body.removeChild(textarea);
}
