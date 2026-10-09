import { useCallback, useEffect, useState } from "react";
import {
  settingsClearDashscopeCredentials,
  settingsClearDoubaoCredentials,
  settingsClearTosCredentials,
  settingsGet,
  settingsTestDashscope,
  settingsTestDoubao,
  settingsTestTos,
  settingsUpdate,
  type AppError,
  type SettingsTestDashscopeOverrides,
  type SettingsTestDoubaoOverrides,
  type SettingsTestTosOverrides,
} from "../../ipc";
import { friendlyErrorMessage } from "../../shared/lib";
import { Button, ConfirmDialog } from "../../shared/ui";
import styles from "./SettingsCredentials.module.css";

const SECRET_MASK = "••••••••••••";

type ClearTarget = "doubao" | "dashscope" | "tos" | null;
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
  const [dashscopeConfigured, setDashscopeConfigured] = useState(false);
  const [tosConfigured, setTosConfigured] = useState(false);
  const [doubaoKey, setDoubaoKey] = useState("");
  const [doubaoMasked, setDoubaoMasked] = useState(false);
  const [dashscopeKey, setDashscopeKey] = useState("");
  const [dashscopeMasked, setDashscopeMasked] = useState(false);
  const [tosAk, setTosAk] = useState("");
  const [tosSk, setTosSk] = useState("");
  const [tosAkMasked, setTosAkMasked] = useState(false);
  const [tosSkMasked, setTosSkMasked] = useState(false);
  const [tosRegion, setTosRegion] = useState("");
  const [tosBucket, setTosBucket] = useState("");
  const [tosEndpoint, setTosEndpoint] = useState("");
  const [status, setStatus] = useState<"idle" | "loading" | "saving">("loading");
  const [doubaoError, setDoubaoError] = useState<string | null>(null);
  const [dashscopeError, setDashscopeError] = useState<string | null>(null);
  const [tosError, setTosError] = useState<string | null>(null);
  const [doubaoSavedHint, setDoubaoSavedHint] = useState(false);
  const [dashscopeSavedHint, setDashscopeSavedHint] = useState(false);
  const [tosSavedHint, setTosSavedHint] = useState(false);
  const [doubaoTest, setDoubaoTest] = useState<TestStatus>("idle");
  const [dashscopeTest, setDashscopeTest] = useState<TestStatus>("idle");
  const [tosTest, setTosTest] = useState<TestStatus>("idle");
  const [clearTarget, setClearTarget] = useState<ClearTarget>(null);

  const applySettingsFlags = useCallback(
    (settings: {
      doubao_configured: boolean;
      dashscope_configured: boolean;
      tos_configured: boolean;
      tos_region: string;
      tos_bucket: string;
      tos_endpoint: string;
    }) => {
      setDoubaoConfigured(settings.doubao_configured);
      setDashscopeConfigured(settings.dashscope_configured);
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

      if (settings.dashscope_configured) {
        setDashscopeKey("");
        setDashscopeMasked(true);
      } else {
        setDashscopeKey("");
        setDashscopeMasked(false);
      }

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

  async function saveDashscope() {
    if (dashscopeMasked) return;
    const nextKey = dashscopeKey.trim();
    if (!nextKey || nextKey === SECRET_MASK) return;
    setStatus("saving");
    setDashscopeError(null);
    setDashscopeSavedHint(false);
    setDashscopeTest("idle");
    try {
      const settings = await settingsUpdate({
        dashscope_api_key: nextKey,
      });
      applySettingsFlags(settings);
      if (!settings.dashscope_configured) {
        setDashscopeError(
          "DashScope 密钥未能保存到系统密钥环，请重试或检查 OS 凭据权限",
        );
        return;
      }
      setDashscopeSavedHint(true);
    } catch (err) {
      const appErr = err as AppError;
      setDashscopeError(friendlyErrorMessage(appErr));
    } finally {
      setStatus("idle");
    }
  }

  async function clearDashscope() {
    setStatus("saving");
    setDashscopeError(null);
    setDashscopeSavedHint(false);
    setDashscopeTest("idle");
    try {
      const settings = await settingsClearDashscopeCredentials();
      applySettingsFlags(settings);
      setDashscopeSavedHint(true);
    } catch (err) {
      const appErr = err as AppError;
      setDashscopeError(friendlyErrorMessage(appErr));
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
    } else if (clearTarget === "dashscope") {
      await clearDashscope();
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

  async function testDashscope() {
    setDashscopeTest("testing");
    setDashscopeError(null);
    setDashscopeSavedHint(false);
    const overrides: SettingsTestDashscopeOverrides = {};
    if (isUsableSecret(dashscopeMasked, dashscopeKey)) {
      overrides.dashscope_api_key = dashscopeKey.trim();
    }
    try {
      await settingsTestDashscope(overrides);
      setDashscopeTest("ok");
    } catch (err) {
      setDashscopeTest("idle");
      setDashscopeError(friendlyErrorMessage(err as AppError));
    }
  }

  const canSaveDoubao =
    !doubaoMasked &&
    doubaoKey.trim().length > 0 &&
    doubaoKey.trim() !== SECRET_MASK &&
    status !== "saving" &&
    status !== "loading";

  const canSaveDashscope =
    !dashscopeMasked &&
    dashscopeKey.trim().length > 0 &&
    dashscopeKey.trim() !== SECRET_MASK &&
    status !== "saving" &&
    status !== "loading";

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

  const dashscopeKeyReady =
    isUsableSecret(dashscopeMasked, dashscopeKey) || dashscopeConfigured;
  const dashscopeTestIncomplete = !dashscopeKeyReady;
  const canTestDashscope =
    !dashscopeTestIncomplete &&
    dashscopeTest !== "testing" &&
    status !== "loading";

  const clearCopy =
    clearTarget === "doubao"
      ? {
          title: "清除豆包 API Key",
          description: "确定清除已保存的豆包 API Key？清除后需重新填写才能转写。",
        }
      : clearTarget === "dashscope"
        ? {
            title: "清除 DashScope API Key",
            description: "确定清除已保存的通义千问 / DashScope API Key？清除后需重新填写才能生成摘要。",
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
        <h2>通义千问 / DashScope（摘要）</h2>
        <p className={styles.hint}>
          API Key 保存在本机密钥存储中，不会回传明文。摘要使用 qwen3.7-plus
          模型。
        </p>
        <p
          className={`${styles.status} ${dashscopeConfigured ? styles.statusOk : styles.statusWarn}`}
        >
          {dashscopeConfigured ? "已配置 DashScope API Key" : "尚未配置 DashScope API Key"}
        </p>
        <div className={styles.fields}>
          <label>
            API Key
            <input
              type="password"
              autoComplete="off"
              value={secretDisplay(dashscopeMasked, dashscopeKey)}
              onFocus={() =>
                clearMask(dashscopeMasked, setDashscopeMasked, setDashscopeKey)
              }
              onChange={(e) => {
                onSecretChange(
                  dashscopeMasked,
                  setDashscopeMasked,
                  setDashscopeKey,
                  e.target.value,
                );
                setDashscopeSavedHint(false);
                setDashscopeTest("idle");
              }}
              placeholder="DashScope API Key"
            />
          </label>
        </div>
        <div className={styles.actions}>
          <Button
            onClick={() => void saveDashscope()}
            disabled={!canSaveDashscope}
          >
            {status === "saving" ? "保存中…" : "保存 API Key"}
          </Button>
          <Button
            variant="secondary"
            onClick={() => setClearTarget("dashscope")}
            disabled={
              !dashscopeConfigured || status === "saving" || status === "loading"
            }
          >
            清除 API Key
          </Button>
          <Button
            variant="secondary"
            onClick={() => void testDashscope()}
            disabled={!canTestDashscope}
            title={
              dashscopeTestIncomplete
                ? "请填写 API Key，或先保存后再测试"
                : undefined
            }
          >
            {dashscopeTest === "testing" ? "测试中…" : "测试连接"}
          </Button>
          {dashscopeSavedHint && dashscopeTest !== "ok" && (
            <span className={styles.ok}>已更新</span>
          )}
          {dashscopeTest === "ok" && (
            <span className={styles.ok}>连接正常</span>
          )}
        </div>
        {dashscopeError && <p className={styles.error}>{dashscopeError}</p>}
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
