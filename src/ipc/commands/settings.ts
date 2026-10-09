import { invokeCommand } from "../client";
import type {
  Settings,
  SettingsTestDashscopeOverrides,
  SettingsTestDoubaoOverrides,
  SettingsTestResult,
  SettingsTestTosOverrides,
  SettingsUpdate,
} from "../types";

export function settingsGet(): Promise<Settings> {
  return invokeCommand<Settings>("settings_get");
}

export function settingsUpdate(update: SettingsUpdate): Promise<Settings> {
  return invokeCommand<Settings>("settings_update", { update });
}

export function settingsClearDoubaoCredentials(): Promise<Settings> {
  return invokeCommand<Settings>("settings_clear_doubao_credentials");
}

export function settingsClearDashscopeCredentials(): Promise<Settings> {
  return invokeCommand<Settings>("settings_clear_dashscope_credentials");
}

export function settingsClearTosCredentials(): Promise<Settings> {
  return invokeCommand<Settings>("settings_clear_tos_credentials");
}

export function settingsTestDoubao(
  overrides: SettingsTestDoubaoOverrides = {},
): Promise<SettingsTestResult> {
  return invokeCommand<SettingsTestResult>("settings_test_doubao", {
    doubao_api_key: overrides.doubao_api_key,
  });
}

export function settingsTestTos(
  overrides: SettingsTestTosOverrides = {},
): Promise<SettingsTestResult> {
  return invokeCommand<SettingsTestResult>("settings_test_tos", {
    tos_access_key_id: overrides.tos_access_key_id,
    tos_secret_access_key: overrides.tos_secret_access_key,
    tos_region: overrides.tos_region,
    tos_bucket: overrides.tos_bucket,
    tos_endpoint: overrides.tos_endpoint,
  });
}

export function settingsTestDashscope(
  overrides: SettingsTestDashscopeOverrides = {},
): Promise<SettingsTestResult> {
  return invokeCommand<SettingsTestResult>("settings_test_dashscope", {
    dashscope_api_key: overrides.dashscope_api_key,
  });
}
