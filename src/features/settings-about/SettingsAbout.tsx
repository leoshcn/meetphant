import { useState } from "react";
import { logsOpenDir, type AppError } from "../../ipc";
import { friendlyErrorMessage } from "../../shared/lib";
import { Button } from "../../shared/ui";
import { useAppUpdate } from "../app-update";
import styles from "./SettingsAbout.module.css";

export function SettingsAboutPanel() {
  const [logsError, setLogsError] = useState<string | null>(null);

  async function openLogs() {
    setLogsError(null);
    try {
      await logsOpenDir();
    } catch (err) {
      setLogsError(friendlyErrorMessage(err as AppError));
    }
  }

  const {
    phase,
    currentVersion,
    availableVersion,
    notes,
    error,
    progressPercent,
    appBusy,
    canInstall,
    checkManual,
    download,
    installAndRelaunch,
    downloadInstallAndRelaunch,
  } = useAppUpdate();

  const checking = phase === "checking";
  const downloading = phase === "downloading";
  const installing = phase === "installing";
  const hasUpdate =
    phase === "available" ||
    phase === "downloading" ||
    phase === "readyToInstall" ||
    phase === "installing";

  let statusText = "尚未检查更新。";
  if (checking) statusText = "正在检查…";
  else if (phase === "upToDate") statusText = "已是最新版本。";
  else if (phase === "available" && availableVersion) {
    statusText = `发现新版本 ${availableVersion}。`;
  } else if (downloading) {
    statusText =
      progressPercent != null
        ? `正在下载… ${progressPercent}%`
        : "正在下载…";
  } else if (phase === "readyToInstall" && availableVersion) {
    statusText = `${availableVersion} 已下载，可以安装并重启。`;
  } else if (installing) statusText = "正在安装并准备重启…";
  else if (phase === "error") statusText = "检查或更新失败。";

  return (
    <section className={styles.panel}>
      <h2>关于 Meetphant</h2>
      <p className={styles.hint}>
        查看当前版本，并从 GitHub Release 检查精简安装包更新。录音或转写进行中时不能安装重启。
      </p>

      <dl className={styles.meta}>
        <div>
          <dt>当前版本</dt>
          <dd>{currentVersion ?? "…"}</dd>
        </div>
        {availableVersion ? (
          <div>
            <dt>可用版本</dt>
            <dd>{availableVersion}</dd>
          </div>
        ) : null}
      </dl>

      <p className={styles.status} role="status">
        {statusText}
      </p>

      {notes ? (
        <div className={styles.notes}>
          <h3>更新说明</h3>
          <pre>{notes}</pre>
        </div>
      ) : null}

      {appBusy && hasUpdate ? (
        <p className={styles.warn}>
          正在录音或转写：可以下载更新，请空闲后再安装。
        </p>
      ) : null}

      {error ? <p className={styles.error}>{error}</p> : null}

      <div className={styles.actions}>
        <Button
          type="button"
          variant="secondary"
          disabled={checking || downloading || installing}
          onClick={() => void checkManual()}
        >
          检查更新
        </Button>
        {phase === "available" ? (
          <>
            <Button
              type="button"
              variant="secondary"
              disabled={downloading || installing}
              onClick={() => void download()}
            >
              仅下载
            </Button>
            <Button
              type="button"
              disabled={!canInstall || downloading || installing}
              onClick={() => void downloadInstallAndRelaunch()}
            >
              下载并安装
            </Button>
          </>
        ) : null}
        {phase === "readyToInstall" ? (
          <Button
            type="button"
            disabled={!canInstall || installing}
            onClick={() => void installAndRelaunch()}
          >
            安装并重启
          </Button>
        ) : null}
      </div>

      <p className={styles.hint}>
        遇到问题时，可以打开日志文件夹，把最近的日志文件发给我们。日志不包含密钥。
      </p>
      {logsError ? <p className={styles.error}>{logsError}</p> : null}
      <div className={styles.actions}>
        <Button type="button" variant="secondary" onClick={() => void openLogs()}>
          打开日志文件夹
        </Button>
      </div>
    </section>
  );
}
