import { describe, expect, it } from "vitest";
import {
  SUMMARY_LLM_PRESETS,
  findSummaryLlmPreset,
  summaryLlmEndpointChanged,
  summaryLlmFormForProvider,
  validateSummaryLlmBaseUrl,
} from "./summaryLlm";

describe("summary LLM presets", () => {
  it("includes all backend preset ids", () => {
    expect(SUMMARY_LLM_PRESETS.map((p) => p.id)).toEqual([
      "dashscope",
      "deepseek",
      "openai",
      "moonshot",
      "zhipu",
      "ark",
      "custom",
    ]);
  });

  it("dashscope keeps the pre-upgrade defaults", () => {
    expect(summaryLlmFormForProvider("dashscope")).toEqual({
      baseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1",
      model: "qwen3.7-plus",
    });
  });

  it("presets without a recommended model leave the model empty", () => {
    expect(summaryLlmFormForProvider("openai").model).toBe("");
    expect(summaryLlmFormForProvider("custom")).toEqual({ baseUrl: "", model: "" });
  });

  it("unknown ids resolve to custom", () => {
    expect(findSummaryLlmPreset("future").id).toBe("custom");
  });
});

describe("summaryLlmEndpointChanged", () => {
  const saved = {
    provider: "dashscope",
    baseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1",
  };

  it("ignores trailing slash and whitespace", () => {
    expect(
      summaryLlmEndpointChanged(saved, {
        provider: "dashscope",
        baseUrl: " https://dashscope.aliyuncs.com/compatible-mode/v1/ ",
      }),
    ).toBe(false);
  });

  it("detects provider or base URL changes", () => {
    expect(
      summaryLlmEndpointChanged(saved, { ...saved, provider: "deepseek" }),
    ).toBe(true);
    expect(
      summaryLlmEndpointChanged(saved, {
        ...saved,
        baseUrl: "https://proxy.example.com/v1",
      }),
    ).toBe(true);
  });
});

describe("validateSummaryLlmBaseUrl", () => {
  it("accepts http(s) URLs", () => {
    expect(validateSummaryLlmBaseUrl("https://api.deepseek.com/v1")).toBeNull();
    expect(validateSummaryLlmBaseUrl("http://localhost:11434/v1")).toBeNull();
  });

  it("rejects empty, non-http, and malformed values", () => {
    expect(validateSummaryLlmBaseUrl("")).not.toBeNull();
    expect(validateSummaryLlmBaseUrl("   ")).not.toBeNull();
    expect(validateSummaryLlmBaseUrl("ftp://x")).not.toBeNull();
    expect(validateSummaryLlmBaseUrl("api.deepseek.com")).not.toBeNull();
    expect(validateSummaryLlmBaseUrl("https://a b.com")).not.toBeNull();
  });
});
