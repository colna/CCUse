import type { ProviderInput } from "@/lib/tauri";

export type ProviderKind = ProviderInput["kind"];

/** 添加 / 编辑供应商表单的下拉选项元数据。 */
export interface ProviderKindOption {
  kind: ProviderKind;
  label: string;
  /** 选中此类型时，自动回填到 Base URL 输入框的官方端点；空串表示由用户手填。 */
  defaultBaseUrl: string;
  /** 是否必须填写 Base URL（中转 / 自定义类型为 true）。 */
  requiresBaseUrl: boolean;
  /** 后端代理是否已实现该类型；false 时禁用按钮 / 选项，避免误选。 */
  supported: boolean;
}

export const PROVIDER_KIND_OPTIONS: readonly ProviderKindOption[] = [
  {
    kind: "openai",
    label: "OpenAI",
    defaultBaseUrl: "https://api.openai.com",
    requiresBaseUrl: false,
    supported: true,
  },
  {
    kind: "anthropic",
    label: "Anthropic",
    defaultBaseUrl: "https://api.anthropic.com",
    requiresBaseUrl: false,
    supported: true,
  },
  {
    kind: "gemini",
    label: "Gemini",
    defaultBaseUrl: "https://generativelanguage.googleapis.com",
    requiresBaseUrl: false,
    supported: false,
  },
  {
    kind: "relay",
    label: "Relay",
    defaultBaseUrl: "",
    requiresBaseUrl: true,
    supported: true,
  },
  {
    kind: "custom",
    label: "Custom",
    defaultBaseUrl: "",
    requiresBaseUrl: true,
    supported: true,
  },
] as const;
