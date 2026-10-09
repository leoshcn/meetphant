import { beforeEach, describe, expect, it, vi } from "vitest";
import { __setInvokeForTests } from "../client";
import {
  settingsClearDashscopeCredentials,
  settingsClearDoubaoCredentials,
  settingsClearTosCredentials,
  settingsGet,
  settingsTestDashscope,
  settingsTestDoubao,
  settingsTestTos,
  settingsUpdate,
} from "./settings";

const emptySettings = {
  hotwords: [] as string[],
  context_text: "",
  doubao_configured: false,
  dashscope_configured: false,
  tos_configured: false,
  tos_region: "",
  tos_bucket: "",
  tos_endpoint: "",
  recording_dir: "",
  recording_dir_resolved: "C:\\Users\\test\\Documents\\Meetphant\\Recordings",
  theme_preference: "system" as const,
};

describe("settings commands", () => {
  beforeEach(() => {
    __setInvokeForTests(null);
  });

  it("settingsGet invokes settings_get", async () => {
    const invoke = vi.fn().mockResolvedValue({
      ...emptySettings,
      hotwords: ["Meetphant"],
    });
    __setInvokeForTests(invoke);

    const result = await settingsGet();
    expect(invoke).toHaveBeenCalledWith("settings_get", undefined);
    expect(result.hotwords).toEqual(["Meetphant"]);
    expect(result.doubao_configured).toBe(false);
    expect(result.dashscope_configured).toBe(false);
    expect(result.tos_configured).toBe(false);
  });

  it("settingsUpdate passes SettingsUpdate payload", async () => {
    const invoke = vi.fn().mockResolvedValue({
      ...emptySettings,
      hotwords: ["Meetphant"],
      context_text: "ctx",
      doubao_configured: true,
      dashscope_configured: true,
      tos_configured: true,
      tos_region: "cn-beijing",
      tos_bucket: "meetphant",
    });
    __setInvokeForTests(invoke);

    await settingsUpdate({
      hotwords: ["Meetphant"],
      context_text: "ctx",
      doubao_api_key: "doubao-key",
      dashscope_api_key: "sk-test",
      tos_access_key_id: "ak",
      tos_secret_access_key: "sk",
      tos_region: "cn-beijing",
      tos_bucket: "meetphant",
    });
    expect(invoke).toHaveBeenCalledWith("settings_update", {
      update: {
        hotwords: ["Meetphant"],
        context_text: "ctx",
        doubao_api_key: "doubao-key",
        dashscope_api_key: "sk-test",
        tos_access_key_id: "ak",
        tos_secret_access_key: "sk",
        tos_region: "cn-beijing",
        tos_bucket: "meetphant",
      },
    });
  });

  it("settingsClearDoubaoCredentials invokes clear command", async () => {
    const invoke = vi.fn().mockResolvedValue(emptySettings);
    __setInvokeForTests(invoke);
    await settingsClearDoubaoCredentials();
    expect(invoke).toHaveBeenCalledWith(
      "settings_clear_doubao_credentials",
      undefined,
    );
  });

  it("settingsClearDashscopeCredentials invokes clear command", async () => {
    const invoke = vi.fn().mockResolvedValue(emptySettings);
    __setInvokeForTests(invoke);
    await settingsClearDashscopeCredentials();
    expect(invoke).toHaveBeenCalledWith(
      "settings_clear_dashscope_credentials",
      undefined,
    );
  });

  it("settingsClearTosCredentials invokes clear command", async () => {
    const invoke = vi.fn().mockResolvedValue(emptySettings);
    __setInvokeForTests(invoke);
    await settingsClearTosCredentials();
    expect(invoke).toHaveBeenCalledWith(
      "settings_clear_tos_credentials",
      undefined,
    );
  });

  it("settingsTestDoubao passes optional overrides", async () => {
    const invoke = vi.fn().mockResolvedValue({ ok: true });
    __setInvokeForTests(invoke);
    await settingsTestDoubao({ doubao_api_key: "doubao-key" });
    expect(invoke).toHaveBeenCalledWith("settings_test_doubao", {
      doubao_api_key: "doubao-key",
    });
  });

  it("settingsTestDoubao allows empty overrides", async () => {
    const invoke = vi.fn().mockResolvedValue({ ok: true });
    __setInvokeForTests(invoke);
    await settingsTestDoubao();
    expect(invoke).toHaveBeenCalledWith("settings_test_doubao", {
      doubao_api_key: undefined,
    });
  });

  it("settingsTestTos passes optional overrides", async () => {
    const invoke = vi.fn().mockResolvedValue({ ok: true });
    __setInvokeForTests(invoke);
    await settingsTestTos({
      tos_access_key_id: "ak",
      tos_region: "cn-beijing",
      tos_bucket: "b",
    });
    expect(invoke).toHaveBeenCalledWith("settings_test_tos", {
      tos_access_key_id: "ak",
      tos_secret_access_key: undefined,
      tos_region: "cn-beijing",
      tos_bucket: "b",
      tos_endpoint: undefined,
    });
  });

  it("settingsTestDashscope passes optional override", async () => {
    const invoke = vi.fn().mockResolvedValue({ ok: true });
    __setInvokeForTests(invoke);
    await settingsTestDashscope({ dashscope_api_key: "sk-x" });
    expect(invoke).toHaveBeenCalledWith("settings_test_dashscope", {
      dashscope_api_key: "sk-x",
    });
  });
});
