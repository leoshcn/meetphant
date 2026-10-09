import { useCallback, useEffect, useState } from "react";
import {
  settingsClearDoubaoCredentials,
  settingsClearSummaryLlmCredentials,
  settingsClearTosCredentials,
  settingsGet,
  settingsTestDoubao,
  settingsTestSummaryLlm,
  settingsTestTos,
  settingsUpdate,
  type AppError,
  type Settings,
  type SettingsTestDoubaoOverrides,
  type SettingsTestSummaryLlmOverrides,
  type SettingsTestTosOverrides,
  type SettingsUpdate,
  type SummaryLlmProvider,
} from "../../ipc";
import { friendlyErrorMessage } from "../../shared/lib";
import { Button, ConfirmDialog } from "../../shared/ui";
import styles from "./SettingsCredentials.module.css";
import {
  SUMMARY_LLM_PRESETS,
  findSummaryLlmPreset,
  summaryLlmEndpointChanged,
  summaryLlmFormForProvider,
  validateSummaryLlmBaseUrl,
} from "./summaryLlm";

const SECRET_MASK = "••••••••••••";

type ClearTarget = "doubao" | "summaryLlm" | "tos" | null;

type SavedSummaryLlm = {
  provider: SummaryLlmProvider;
  baseUrl: string;
  model: string;
};

const SUMMARY_LLM_KEY_REQUIRED_HINT =
  "已切换服务商或修改 Base URL，请重新填写 API Key 后再保存或测试";
type TestStatus = "idle" | "testing" | "ok";

function secretDisplay(masked: boolean, value: string): string {
  return masked ? SECRET_MASK : value;
}

function isUsableSecret(masked: boolean, value: string): boolean {
  if (masked) return false;
  const trimmed = value.trim();
  return trimmed.length > 0 && trimmed !== SECRET_MASK;
}

export function SettingsCredentialsPanel() {
  const [doubaoConfigured, setDoubaoConfigured] = useState(false);
  const [llmConfigured, setLlmConfigured] = useState(false);
  const [tosConfigured, setTosConfigured] = useState(false);
  const [doubaoKey, setDoubaoKey] = useState("");
  const [doubaoMasked, setDoubaoMasked] = useState(false);
  const [llmSaved, setLlmSaved] = useState<SavedSummaryLlm>({
    provider: "dashscope",
    baseUrl: "",
    model: "",
  });
  const [llmProvider, setLlmProvider] = useState<SummaryLlmProvider>("dashscope");
  const [llmBaseUrl, setLlmBaseUrl] = useState("");
  const [llmModel, setLlmModel] = useState("");
  const [llmKey, setLlmKey] = useState("");
  const [llmMasked, setLlmMasked] = useState(false);
  const [tosAk, setTosAk] = useState("");
  const [tosSk, setTosSk] = useState("");
  const [tosAkMasked, setTosAkMasked] = useState(false);
  const [tosSkMasked, setTosSkMasked] = useState(false);
  const [tosRegion, setTosRegion] = useState("");
  const [tosBucket, setTosBucket] = useState("");
  const [tosEndpoint, setTosEndpoint] = useState("");
  const [status, setStatus] = useState<"idle" | "loading" | "saving">("loading");
  const [doubaoError, setDoubaoError] = useState<string | null>(null);
  const [llmError, setLlmError] = useState<string | null>(null);
  const [tosError, setTosError] = useState<string | null>(null);
  const [doubaoSavedHint, setDoubaoSavedHint] = useState(false);
  const [llmSavedHint, setLlmSavedHint] = useState(false);
  const [tosSavedHint, setTosSavedHint] = useState(false);
  const [doubaoTest, setDoubaoTest] = useState<TestStatus>("idle");
  const [llmTest, setLlmTest] = useState<TestStatus>("idle");
  const [tosTest, setTosTest] = useState<TestStatus>("idle");
  const [clearTarget, setClearTarget] = useState<ClearTarget>(null);

  const applySettingsFlags = useCallback(
    (settings: Settings) => {
      setDoubaoConfigured(settings.doubao_configured);
      setLlmConfigured(settings.summary_llm_configured);
      setTosConfigured(settings.tos_configured);
      setTosRegion(settings.tos_region);
      setTosBucket(settings.tos_bucket);
      setTosEndpoint(settings.tos_endpoint);

      if (settings.doubao_configured) {
        setDoubaoKey("");
        setDoubaoMasked(true);
      } else {
        setDoubaoKey("");
        setDoubaoMasked(false);
      }

      const llmProviderId = findSummaryLlmPreset(settings.summary_llm_provider).id;
      setLlmSaved({
        provider: llmProviderId,
        baseUrl: settings.summary_llm_base_url,
        model: settings.summary_llm_model,
      });
      setLlmProvider(llmProviderId);
      setLlmBaseUrl(settings.summary_llm_base_url);
      setLlmModel(settings.summary_llm_model);
      setLlmKey("");
      setLlmMasked(settings.summary_llm_configured);

      if (settings.tos_configured) {
        setTosAk("");
        setTosSk("");
        setTosAkMasked(true);
        setTosSkMasked(true);
      } else {
        setTosAk("");
        setTosSk("");
        setTosAkMasked(false);
        setTosSkMasked(false);
      }
    },
    [],
  );

  const refresh = useCallback(async () => {
    const settings = await settingsGet();
    applySettingsFlags(settings);
  }, [applySettingsFlags]);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        await refresh();
        if (!cancelled) {
          setStatus("idle");
        }
      } catch (err) {
        if (!cancelled) {
          const appErr = err as AppError;
          setDoubaoError(friendlyErrorMessage(appErr));
          setStatus("idle");
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [refresh]);

  function clearMask(
    masked: boolean,
    setMasked: (v: boolean) => void,
    setValue: (v: string) => void,
  ) {
    if (masked) {
      setMasked(false);
      setValue("");
    }
  }

  function onSecretChange(
    masked: boolean,
    setMasked: (v: boolean) => void,
    setValue: (v: string) => void,
    next: string,
  ) {
    if (masked) {
      setMasked(false);
      if (next.startsWith(SECRET_MASK)) {
        setValue(next.slice(SECRET_MASK.length));
      } else if (next.length < SECRET_MASK.length) {
        setValue("");
      } else {
        setValue(next);
      }
      return;
    }
    setValue(next);
  }

  async function saveDoubao() {
    if (doubaoMasked) return;
    const nextKey = doubaoKey.trim();
    if (!nextKey || nextKey === SECRET_MASK) return;
    setStatus("saving");
    setDoubaoError(null);
    setDoubaoSavedHint(false);
    setDoubaoTest("idle");
    try {
      const settings = await settingsUpdate({
        doubao_api_key: nextKey,
      });
      applySettingsFlags(settings);
      if (!settings.doubao_configured) {
        setDoubaoError("凭证未能保存到系统密钥环，请重试或检查 OS 凭据权限");
        return;
      }
      setDoubaoSavedHint(true);
    } catch (err) {
      const appErr = err as AppError;
      setDoubaoError(friendlyErrorMessage(appErr));
    } finally {
      setStatus("idle");
    }
  }

  async function clearDoubao() {
    setStatus("saving");
    setDoubaoError(null);
    setDoubaoSavedHint(false);
    setDoubaoTest("idle");
    try {
      const settings = await settingsClearDoubaoCredentials();
      applySettingsFlags(settings);
      setDoubaoSavedHint(true);
    } catch (err) {
      const appErr = err as AppError;
      setDoubaoError(friendlyErrorMessage(appErr));
    } finally {
      setStatus("idle");
      setClearTarget(null);
    }
  }

  function resetLlmFeedback() {
    setLlmSavedHint(false);
    setLlmTest("idle");
    setLlmError(null);
  }

  function onLlmProviderChange(next: SummaryLlmProvider) {
    setLlmProvider(next);
    if (next === llmSaved.provider) {
      // Back to the saved provider: restore saved values; the saved key applies again.
      setLlmBaseUrl(llmSaved.baseUrl);
      setLlmModel(llmSaved.model);
      setLlmMasked(llmConfigured);
    } else {
      const form = summaryLlmFormForProvider(next);
      setLlmBaseUrl(form.baseUrl);
      setLlmModel(form.model);
      setLlmMasked(false);
    }
    // D5: a key typed for one provider is never carried over to another.
    setLlmKey("");
    resetLlmFeedback();
  }

  function onLlmBaseUrlChange(next: string) {
    setLlmBaseUrl(next);
    // D5: the saved key must not be sent to a different address.
    if (
      llmMasked &&
      summaryLlmEndpointChanged(llmSaved, { provider: llmProvider, baseUrl: next })
    ) {
      setLlmMasked(false);
      setLlmKey("");
    }
    resetLlmFeedback();
  }

  async function saveSummaryLlm() {
    const typedKey = isUsableSecret(llmMasked, llmKey);
    setStatus("saving");
    resetLlmFeedback();
    try {
      const update: SettingsUpdate = {
        summary_llm_provider: llmProvider,
        summary_llm_base_url: llmBaseUrl.trim(),
        summary_llm_model: llmModel.trim(),
      };
      if (typedKey) {
        update.summary_llm_api_key = llmKey.trim();
      }
      const settings = await settingsUpdate(update);
      applySettingsFlags(settings);
      if (typedKey && !settings.summary_llm_configured) {
        setLlmError("API Key 未能保存到系统密钥环，请重试或检查 OS 凭据权限");
        return;
      }
      setLlmSavedHint(true);
    } catch (err) {
      const appErr = err as AppError;
      setLlmError(friendlyErrorMessage(appErr));
    } finally {
      setStatus("idle");
    }
  }

  async function clearSummaryLlm() {
    setStatus("saving");
    resetLlmFeedback();
    try {
      const settings = await settingsClearSummaryLlmCredentials();
      applySettingsFlags(settings);
      setLlmSavedHint(true);
    } catch (err) {
      const appErr = err as AppError;
      setLlmError(friendlyErrorMessage(appErr));
    } finally {
      setStatus("idle");
      setClearTarget(null);
    }
  }

  async function saveTos() {
    setStatus("saving");
    setTosError(null);
    setTosSavedHint(false);
    setTosTest("idle");
    try {
      const update: Parameters<typeof settingsUpdate>[0] = {
        tos_region: tosRegion,
        tos_bucket: tosBucket,
        tos_endpoint: tosEndpoint,
      };
      // Masked placeholders must never be submitted; only a full non-masked pair.
      if (
        !tosAkMasked &&
        !tosSkMasked &&
        tosAk.trim().length > 0 &&
        tosSk.trim().length > 0 &&
        tosAk.trim() !== SECRET_MASK &&
        tosSk.trim() !== SECRET_MASK
      ) {
        update.tos_access_key_id = tosAk.trim();
        update.tos_secret_access_key = tosSk.trim();
      }
      const settings = await settingsUpdate(update);
      applySettingsFlags(settings);
      if (!settings.tos_configured) {
        setTosError(
          "TOS 未完整配置：需要 Access Key、Secret Key、Region 与 Bucket",
        );
        return;
      }
      setTosSavedHint(true);
    } catch (err) {
      const appErr = err as AppError;
      setTosError(friendlyErrorMessage(appErr));
    } finally {
      setStatus("idle");
    }
  }

  async function clearTos() {
    setStatus("saving");
    setTosError(null);
    setTosSavedHint(false);
    setTosTest("idle");
    try {
      const settings = await settingsClearTosCredentials();
      applySettingsFlags(settings);
      setTosSavedHint(true);
    } catch (err) {
      const appErr = err as AppError;
      setTosError(friendlyErrorMessage(appErr));
    } finally {
      setStatus("idle");
      setClearTarget(null);
    }
  }

  async function confirmClear() {
    if (clearTarget === "doubao") {
      await clearDoubao();
    } else if (clearTarget === "summaryLlm") {
      await clearSummaryLlm();
    } else if (clearTarget === "tos") {
      await clearTos();
    }
  }

  async function testDoubao() {
    setDoubaoTest("testing");
    setDoubaoError(null);
    setDoubaoSavedHint(false);
    const overrides: SettingsTestDoubaoOverrides = {};
    if (isUsableSecret(doubaoMasked, doubaoKey)) {
      overrides.doubao_api_key = doubaoKey.trim();
    }
    try {
      await settingsTestDoubao(overrides);
      setDoubaoTest("ok");
    } catch (err) {
      setDoubaoTest("idle");
      setDoubaoError(friendlyErrorMessage(err as AppError));
    }
  }

  async function testTos() {
    setTosTest("testing");
    setTosError(null);
    setTosSavedHint(false);
    const overrides: SettingsTestTosOverrides = {};
    if (isUsableSecret(tosAkMasked, tosAk)) {
      overrides.tos_access_key_id = tosAk.trim();
    }
    if (isUsableSecret(tosSkMasked, tosSk)) {
      overrides.tos_secret_access_key = tosSk.trim();
    }
    if (tosRegion.trim().length > 0) {
      overrides.tos_region = tosRegion.trim();
    }
    if (tosBucket.trim().length > 0) {
      overrides.tos_bucket = tosBucket.trim();
    }
    if (tosEndpoint.trim().length > 0) {
      overrides.tos_endpoint = tosEndpoint.trim();
    }
    try {
      await settingsTestTos(overrides);
      setTosTest("ok");
    } catch (err) {
      setTosTest("idle");
      setTosError(friendlyErrorMessage(err as AppError));
    }
  }

  async function testSummaryLlm() {
    setLlmTest("testing");
    setLlmError(null);
    setLlmSavedHint(false);
    const overrides: SettingsTestSummaryLlmOverrides = {
      provider: llmProvider,
      base_url: llmBaseUrl.trim(),
      model: llmModel.trim(),
    };
    if (isUsableSecret(llmMasked, llmKey)) {
      overrides.api_key = llmKey.trim();
    }
    try {
      await settingsTestSummaryLlm(overrides);
      setLlmTest("ok");
    } catch (err) {
      setLlmTest("idle");
      setLlmError(friendlyErrorMessage(err as AppError));
    }
  }

  const canSaveDoubao =
    !doubaoMasked &&
    doubaoKey.trim().length > 0 &&
    doubaoKey.trim() !== SECRET_MASK &&
    status !== "saving" &&
    status !== "loading";

  const llmPreset = findSummaryLlmPreset(llmProvider);
  const llmSavedLabel = findSummaryLlmPreset(llmSaved.provider).label;
  const llmTypedKey = isUsableSecret(llmMasked, llmKey);
  const llmEndpointChanged = summaryLlmEndpointChanged(llmSaved, {
    provider: llmProvider,
    baseUrl: llmBaseUrl,
  });
  // D5: after switching provider / base URL a fresh key is required.
  const llmKeyRequired = llmEndpointChanged && !llmTypedKey;
  const llmBaseUrlError = validateSummaryLlmBaseUrl(llmBaseUrl);
  const llmModelMissing = llmModel.trim().length === 0;
  const llmFormInvalid = llmBaseUrlError !== null || llmModelMissing;
  const llmDirty =
    llmTypedKey ||
    llmEndpointChanged ||
    llmModel.trim() !== llmSaved.model.trim();
  const canSaveLlm =
    llmDirty &&
    !llmKeyRequired &&
    !llmFormInvalid &&
    status !== "saving" &&
    status !== "loading";
  const llmFormHint = llmKeyRequired
    ? SUMMARY_LLM_KEY_REQUIRED_HINT
    : (llmBaseUrlError ?? (llmModelMissing ? "请填写模型名" : null));

  const tosSecretsPairReady =
    !tosAkMasked &&
    !tosSkMasked &&
    tosAk.trim().length > 0 &&
    tosSk.trim().length > 0 &&
    tosAk.trim() !== SECRET_MASK &&
    tosSk.trim() !== SECRET_MASK;
  const tosSecretsUntouched = tosAkMasked && tosSkMasked;
  const tosSecretsClearedAfterFocus =
    !tosAkMasked &&
    !tosSkMasked &&
    tosAk.trim().length === 0 &&
    tosSk.trim().length === 0;
  // Allow region-only updates when secrets stay masked (or both cleared after focus);
  // block partial unmask so we never send one key with an empty partner.
  const tosSecretsReady =
    tosSecretsPairReady ||
    (tosConfigured && (tosSecretsUntouched || tosSecretsClearedAfterFocus));

  const canSaveTos =
    tosRegion.trim().length > 0 &&
    tosBucket.trim().length > 0 &&
    tosSecretsReady &&
    status !== "saving" &&
    status !== "loading";

  // Merge-ready: form non-empty secret OR saved (configured). Empty unmasked → backend uses keyring.
  const doubaoKeyReady =
    isUsableSecret(doubaoMasked, doubaoKey) || doubaoConfigured;
  const doubaoTestIncomplete = !doubaoKeyReady;
  const canTestDoubao =
    !doubaoTestIncomplete &&
    doubaoTest !== "testing" &&
    status !== "loading";

  const tosAkReady = isUsableSecret(tosAkMasked, tosAk) || tosConfigured;
  const tosSkReady = isUsableSecret(tosSkMasked, tosSk) || tosConfigured;
  const tosRegionReady = tosRegion.trim().length > 0 || tosConfigured;
  const tosBucketReady = tosBucket.trim().length > 0 || tosConfigured;
  const tosTestIncomplete = !(
    tosAkReady &&
    tosSkReady &&
    tosRegionReady &&
    tosBucketReady
  );
  const canTestTos =
    !tosTestIncomplete && tosTest !== "testing" && status !== "loading";

  // Saved key is usable only while provider and base URL match the saved config.
  const llmKeyReady = llmTypedKey || (!llmEndpointChanged && llmConfigured);
  const llmTestIncomplete = !llmKeyReady || llmFormInvalid;
  const canTestLlm =
    !llmTestIncomplete && llmTest !== "testing" && status !== "loading";
  const llmTestTitle = !llmKeyReady
    ? llmEndpointChanged
      ? SUMMARY_LLM_KEY_REQUIRED_HINT
      : "请填写 API Key，或先保存后再测试"
    : (llmFormHint ?? undefined);

  const clearCopy =
    clearTarget === "doubao"
      ? {
          title: "清除豆包 API Key",
          description: "确定清除已保存的豆包 API Key？清除后需重新填写才能转写。",
        }
      : clearTarget === "summaryLlm"
        ? {
            title: "清除摘要模型 API Key",
            description:
              "确定清除已保存的摘要模型 API Key？服务商、Base URL 与模型会保留；清除后需重新填写 Key 才能生成摘要。",
          }
        : clearTarget === "tos"
          ? {
              title: "清除 TOS 配置",
              description:
                "确定清除火山 TOS 的密钥与 Region / Bucket 等配置？转写将不可用，直至重新配置。",
            }
          : { title: "", description: "" };

  return (
    <>
      <section className={styles.panel}>
        <h2>豆包凭证（转写）</h2>
        <p className={styles.hint}>
          豆包语音新版控制台 API Key 保存在本机密钥存储中，不会回传明文。使用豆包录音文件识别模型
          2.0，音频需先上传 TOS，请同时配置下方 TOS（上限 512 MiB）。
        </p>
        <p className={styles.hint}>
          已改用新版控制台 API Key；旧版 App Id / Access Token 不再支持。测试连接只校验 API
          Key 与 2.0 服务权限，不消耗识别额度。
        </p>
        <p
          className={`${styles.status} ${doubaoConfigured ? styles.statusOk : styles.statusWarn}`}
        >
          {doubaoConfigured ? "已配置豆包 API Key" : "尚未配置豆包 API Key"}
        </p>
        <div className={styles.fields}>
          <label>
            API Key
            <input
              type="password"
              autoComplete="off"
              value={secretDisplay(doubaoMasked, doubaoKey)}
              onFocus={() =>
                clearMask(doubaoMasked, setDoubaoMasked, setDoubaoKey)
              }
              onChange={(e) => {
                onSecretChange(
                  doubaoMasked,
                  setDoubaoMasked,
                  setDoubaoKey,
                  e.target.value,
                );
                setDoubaoSavedHint(false);
                setDoubaoTest("idle");
              }}
              placeholder="Doubao API Key"
            />
          </label>
        </div>
        <div className={styles.actions}>
          <Button onClick={() => void saveDoubao()} disabled={!canSaveDoubao}>
            {status === "saving" ? "保存中…" : "保存 API Key"}
          </Button>
          <Button
            variant="secondary"
            onClick={() => setClearTarget("doubao")}
            disabled={
              !doubaoConfigured || status === "saving" || status === "loading"
            }
          >
            清除 API Key
          </Button>
          <Button
            variant="secondary"
            onClick={() => void testDoubao()}
            disabled={!canTestDoubao}
            title={
              doubaoTestIncomplete
                ? "请填写 API Key，或先保存后再测试"
                : undefined
            }
          >
            {doubaoTest === "testing" ? "测试中…" : "测试连接"}
          </Button>
          {doubaoSavedHint && doubaoTest !== "ok" && (
            <span className={styles.ok}>已更新</span>
          )}
          {doubaoTest === "ok" && <span className={styles.ok}>连接正常</span>}
        </div>
        {doubaoError && <p className={styles.error}>{doubaoError}</p>}
      </section>

      <section className={styles.panel}>
        <h2>火山 TOS（转写必需）</h2>
        <p className={styles.hint}>
          Access Key / Secret Key 保存在本机密钥存储；Region、Bucket
          与可选 Endpoint 保存在本机。所有音频都需经 TOS 上传后转写。
        </p>
        <p
          className={`${styles.status} ${tosConfigured ? styles.statusOk : styles.statusWarn}`}
        >
          {tosConfigured ? "已配置 TOS" : "尚未完整配置 TOS"}
        </p>
        <div className={styles.fields}>
          <label>
            Access Key Id
            <input
              type="password"
              autoComplete="off"
              value={secretDisplay(tosAkMasked, tosAk)}
              onFocus={() => clearMask(tosAkMasked, setTosAkMasked, setTosAk)}
              onChange={(e) => {
                onSecretChange(
                  tosAkMasked,
                  setTosAkMasked,
                  setTosAk,
                  e.target.value,
                );
                setTosSavedHint(false);
                setTosTest("idle");
              }}
              placeholder="TOS Access Key Id"
            />
          </label>
          <label>
            Secret Access Key
            <input
              type="password"
              autoComplete="off"
              value={secretDisplay(tosSkMasked, tosSk)}
              onFocus={() => clearMask(tosSkMasked, setTosSkMasked, setTosSk)}
              onChange={(e) => {
                onSecretChange(
                  tosSkMasked,
                  setTosSkMasked,
                  setTosSk,
                  e.target.value,
                );
                setTosSavedHint(false);
                setTosTest("idle");
              }}
              placeholder="TOS Secret Access Key"
            />
          </label>
          <label>
            Region
            <input
              type="text"
              autoComplete="off"
              value={tosRegion}
              onChange={(e) => {
                setTosRegion(e.target.value);
                setTosSavedHint(false);
                setTosTest("idle");
              }}
              placeholder="例如 cn-beijing"
            />
          </label>
          <label>
            Bucket
            <input
              type="text"
              autoComplete="off"
              value={tosBucket}
              onChange={(e) => {
                setTosBucket(e.target.value);
                setTosSavedHint(false);
                setTosTest("idle");
              }}
              placeholder="Bucket 名称"
            />
          </label>
          <label>
            Endpoint（可选）
            <input
              type="text"
              autoComplete="off"
              value={tosEndpoint}
              onChange={(e) => {
                setTosEndpoint(e.target.value);
                setTosSavedHint(false);
                setTosTest("idle");
              }}
              placeholder="默认按 Region 自动推断"
            />
          </label>
        </div>
        <div className={styles.actions}>
          <Button onClick={() => void saveTos()} disabled={!canSaveTos}>
            {status === "saving" ? "保存中…" : "保存 TOS 配置"}
          </Button>
          <Button
            variant="secondary"
            onClick={() => setClearTarget("tos")}
            disabled={
              (!tosConfigured && !tosRegion && !tosBucket) ||
              status === "saving" ||
              status === "loading"
            }
          >
            清除 TOS 配置
          </Button>
          <Button
            variant="secondary"
            onClick={() => void testTos()}
            disabled={!canTestTos}
            title={
              tosTestIncomplete
                ? "请填写 Access Key、Secret Key、Region 与 Bucket，或先保存后再测试"
                : undefined
            }
          >
            {tosTest === "testing" ? "测试中…" : "测试连接"}
          </Button>
          {tosSavedHint && tosTest !== "ok" && (
            <span className={styles.ok}>已更新</span>
          )}
          {tosTest === "ok" && <span className={styles.ok}>连接正常</span>}
        </div>
        {tosError && <p className={styles.error}>{tosError}</p>}
      </section>

      <section className={styles.panel}>
        <h2>摘要模型</h2>
        <p className={styles.hint}>
          支持 OpenAI 兼容（Chat Completions）接口。选择服务商会自动填入 Base URL
          与推荐模型，均可修改。API Key 保存在本机密钥存储中，不会回传明文。
        </p>
        <p
          className={`${styles.status} ${llmConfigured ? styles.statusOk : styles.statusWarn}`}
        >
          {llmConfigured
            ? `已配置 API Key（${llmSavedLabel}）`
            : "尚未配置摘要模型 API Key"}
        </p>
        <div className={styles.fields}>
          <label>
            服务商
            <select
              value={llmProvider}
              onChange={(e) =>
                onLlmProviderChange(e.target.value as SummaryLlmProvider)
              }
            >
              {SUMMARY_LLM_PRESETS.map((preset) => (
                <option key={preset.id} value={preset.id}>
                  {preset.label}
                </option>
              ))}
            </select>
          </label>
          <label>
            Base URL
            <input
              type="text"
              autoComplete="off"
              value={llmBaseUrl}
              onChange={(e) => onLlmBaseUrlChange(e.target.value)}
              placeholder={
                llmPreset.defaultBaseUrl || "例如 http://localhost:11434/v1"
              }
            />
          </label>
          <label>
            API Key
            <input
              type="password"
              autoComplete="off"
              value={secretDisplay(llmMasked, llmKey)}
              onFocus={() => clearMask(llmMasked, setLlmMasked, setLlmKey)}
              onChange={(e) => {
                onSecretChange(llmMasked, setLlmMasked, setLlmKey, e.target.value);
                resetLlmFeedback();
              }}
              placeholder="API Key"
            />
          </label>
          <label>
            模型
            <input
              type="text"
              autoComplete="off"
              value={llmModel}
              onChange={(e) => {
                setLlmModel(e.target.value);
                resetLlmFeedback();
              }}
              placeholder={
                llmPreset.defaultModel
                  ? `推荐 ${llmPreset.defaultModel}`
                  : "请填写模型名（必填）"
              }
            />
          </label>
        </div>
        {llmFormHint && (llmDirty || llmKeyRequired) && (
          <p className={styles.hint}>{llmFormHint}</p>
        )}
        <div className={styles.actions}>
          <Button onClick={() => void saveSummaryLlm()} disabled={!canSaveLlm}>
            {status === "saving" ? "保存中…" : "保存配置"}
          </Button>
          <Button
            variant="secondary"
            onClick={() => setClearTarget("summaryLlm")}
            disabled={
              !llmConfigured || status === "saving" || status === "loading"
            }
          >
            清除 API Key
          </Button>
          <Button
            variant="secondary"
            onClick={() => void testSummaryLlm()}
            disabled={!canTestLlm}
            title={llmTestTitle}
          >
            {llmTest === "testing" ? "测试中…" : "测试连接"}
          </Button>
          {llmSavedHint && llmTest !== "ok" && (
            <span className={styles.ok}>已更新</span>
          )}
          {llmTest === "ok" && <span className={styles.ok}>连接正常</span>}
        </div>
        {llmError && <p className={styles.error}>{llmError}</p>}
      </section>

      <ConfirmDialog
        open={clearTarget !== null}
        title={clearCopy.title}
        description={clearCopy.description}
        confirmLabel="清除"
        cancelLabel="取消"
        danger
        busy={status === "saving"}
        onConfirm={() => void confirmClear()}
        onCancel={() => {
          if (status !== "saving") setClearTarget(null);
        }}
      />
    </>
  );
}
