import type { SummaryLlmProvider } from "../../ipc";

/**
 * Mirror of the backend presets in
 * `src-tauri/src/providers/openai_compat/presets.rs` (backend is the source of truth).
 */
export type SummaryLlmPreset = {
  id: SummaryLlmProvider;
  label: string;
  defaultBaseUrl: string;
  /** Recommended model; null means the user must fill it in. */
  defaultModel: string | null;
};

export const SUMMARY_LLM_PRESETS: readonly SummaryLlmPreset[] = [
  {
    id: "dashscope",
    label: "阿里云百炼 DashScope",
    defaultBaseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    defaultModel: "qwen3.7-plus",
  },
  {
    id: "deepseek",
    label: "DeepSeek",
    defaultBaseUrl: "https://api.deepseek.com/v1",
    defaultModel: "deepseek-chat",
  },
  {
    id: "openai",
    label: "OpenAI",
    defaultBaseUrl: "https://api.openai.com/v1",
    defaultModel: null,
  },
  {
    id: "moonshot",
    label: "Moonshot（Kimi）",
    defaultBaseUrl: "https://api.moonshot.cn/v1",
    defaultModel: null,
  },
  {
    id: "zhipu",
    label: "智谱 BigModel",
    defaultBaseUrl: "https://open.bigmodel.cn/api/paas/v4",
    defaultModel: null,
  },
  {
    id: "ark",
    label: "火山方舟（豆包）",
    defaultBaseUrl: "https://ark.cn-beijing.volces.com/api/v3",
    defaultModel: null,
  },
  {
    id: "custom",
    label: "自定义（OpenAI 兼容）",
    defaultBaseUrl: "",
    defaultModel: null,
  },
];

const CUSTOM_PRESET = SUMMARY_LLM_PRESETS.find((p) => p.id === "custom")!;

/** Unknown ids resolve to `custom` (same rule as the backend). */
export function findSummaryLlmPreset(id: string): SummaryLlmPreset {
  return SUMMARY_LLM_PRESETS.find((p) => p.id === id) ?? CUSTOM_PRESET;
}

export type SummaryLlmEndpoint = { provider: string; baseUrl: string };

export function normalizeBaseUrl(url: string): string {
  return url.trim().replace(/\/+$/, "");
}

/**
 * True when the form targets a different provider or base URL than the saved
 * config. The saved API key must then not be reused (D5).
 */
export function summaryLlmEndpointChanged(
  saved: SummaryLlmEndpoint,
  form: SummaryLlmEndpoint,
): boolean {
  return (
    saved.provider !== form.provider ||
    normalizeBaseUrl(saved.baseUrl) !== normalizeBaseUrl(form.baseUrl)
  );
}

/** Returns an error message, or null when the base URL is acceptable. */
export function validateSummaryLlmBaseUrl(url: string): string | null {
  const trimmed = url.trim();
  if (!trimmed) return "请填写 Base URL";
  if (/\s/.test(trimmed)) return "Base URL 不能包含空格";
  let parsed: URL;
  try {
    parsed = new URL(trimmed);
  } catch {
    return "Base URL 必须以 http:// 或 https:// 开头";
  }
  if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
    return "Base URL 必须以 http:// 或 https:// 开头";
  }
  if (!parsed.hostname) return "Base URL 缺少主机名";
  return null;
}

/** Form values after the user picks a provider: preset URL + recommended model. */
export function summaryLlmFormForProvider(id: SummaryLlmProvider): {
  baseUrl: string;
  model: string;
} {
  const preset = findSummaryLlmPreset(id);
  return { baseUrl: preset.defaultBaseUrl, model: preset.defaultModel ?? "" };
}
