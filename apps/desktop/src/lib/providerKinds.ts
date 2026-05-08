import type { ProviderInput } from "@/lib/tauri";

export type ProviderKind = ProviderInput["kind"];

export interface ProviderKindOption {
  kind: ProviderKind;
  label: string;
  defaultBaseUrl: string;
  requiresBaseUrl: boolean;
  supported: boolean;
}

export const PROVIDER_KIND_OPTIONS: ProviderKindOption[] = [
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
    supported: false,
  },
  {
    kind: "custom",
    label: "Custom",
    defaultBaseUrl: "",
    requiresBaseUrl: true,
    supported: false,
  },
];

export const SUPPORTED_PROVIDER_KINDS = new Set<ProviderKind>([
  "openai",
  "anthropic",
]);

export function isSupportedProviderKind(kind: ProviderKind): boolean {
  return SUPPORTED_PROVIDER_KINDS.has(kind);
}
