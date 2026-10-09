import { useCallback, useEffect, useId, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { exit } from "@tauri-apps/plugin-process";
import {
  AppUpdateProvider,
  UpdateBanner,
  useAppUpdate,
} from "../features/app-update";
import { recordStop, type AppError } from "../ipc";
import { HomePage } from "../pages/home";
import { SettingsPage, type SettingsTab } from "../pages/settings";
import { errorTitle, friendlyErrorMessage, hideRecorderWidget, hideRecordingTray } from "../shared/lib";
import { Button, IconButton, SettingsGearIcon } from "../shared/ui";
import styles from "./CloseRecordingDialog.module.css";

type Screen = "home" | "settings";

type CloseDialogPhase =
  | { kind: "choices" }
  | { kind: "saved"; path: string }
  | { kind: "error"; message: string; code?: string };

function SettingsGearWithBadge() {
  const { badgeVisible } = useAppUpdate();
  return (
    <span className="app-header-icon-wrap">
      <SettingsGearIcon />
      {badgeVisible ? (
        <span className="app-header-badge" aria-hidden="true" />
      ) : null}
    </span>
  );
}

function CloseRecordingDialog({
  phase,
  busy,
  onStopAndSave,
  onContinue,
  onCancel,
  onAcknowledgeSaved,
  onDismissError,
}: {
  phase: CloseDialogPhase;
  busy: boolean;
  onStopAndSave: () => void;
  onContinue: () => void;
  onCancel: () => void;
  onAcknowledgeSaved: () => void;
  onDismissError: () => void;
}) {
  const titleId = useId();
  const descId = useId();
  const dialogRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    dialogRef.current?.focus();
  }, [phase]);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key !== "Escape" || busy) return;
      event.preventDefault();
      if (phase.kind === "choices") onCancel();
      else if (phase.kind === "error") onDismissError();
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [busy, phase, onCancel, onDismissError]);

  const title =
    phase.kind === "choices"
      ? "正在录音"
      : phase.kind === "saved"
        ? "录音已保存"
        : "无法保存录音";

  const description =
    phase.kind === "choices"
      ? "关闭 Meetphant 前请选择如何处理当前录音。停止并保存只会落盘音频，不会自动创建会议或开始转写；下次可用「导入音频并转写」处理该文件。"
      : phase.kind === "saved"
        ? `音频已保存到：\n${phase.path}\n\n不会自动创建会议或开始转写。下次打开 Meetphant 后，可用「导入音频并转写」处理该文件。`
        : phase.message;

  return (
    <div className={styles.backdrop} role="presentation">
      <div
        ref={dialogRef}
        className={styles.dialog}
        role="alertdialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={descId}
        tabIndex={-1}
      >
        <h2 id={titleId} className={styles.title}>
          {title}
        </h2>
        <p id={descId} className={styles.description}>
          {description}
        </p>
        <div className={styles.actions}>
          {phase.kind === "choices" ? (
            <>
              <Button variant="secondary" onClick={onCancel} disabled={busy}>
                取消关闭
              </Button>
              <Button variant="secondary" onClick={onContinue} disabled={busy}>
                继续录音
              </Button>
              <Button
                variant="danger"
                onClick={onStopAndSave}
                disabled={busy}
              >
                {busy ? "正在保存…" : "停止并保存录音"}
              </Button>
            </>
          ) : null}
          {phase.kind === "saved" ? (
            <Button variant="primary" onClick={onAcknowledgeSaved}>
              退出应用
            </Button>
          ) : null}
          {phase.kind === "error" ? (
            <Button variant="primary" onClick={onDismissError}>
              知道了
            </Button>
          ) : null}
        </div>
      </div>
    </div>
  );
}

function AppShellChrome({
  screen,
  setScreen,
  settingsTab,
  setSettingsTab,
  workspaceBusy,
  setWorkspaceBusy,
  titleSuffix,
  setTitleSuffix,
}: {
  screen: Screen;
  setScreen: (s: Screen) => void;
  settingsTab: SettingsTab;
  setSettingsTab: (t: SettingsTab) => void;
  workspaceBusy: boolean;
  setWorkspaceBusy: (b: boolean) => void;
  titleSuffix: string | null;
  setTitleSuffix: (t: string | null) => void;
}) {
  const [closePhase, setClosePhase] = useState<CloseDialogPhase | null>(null);
  const [closeBusy, setCloseBusy] = useState(false);

  useEffect(() => {
    let unlistenClose: (() => void) | undefined;
    let unlistenFocus: (() => void) | undefined;

    void listen("recording:close-requested", () => {
      setClosePhase({ kind: "choices" });
      setCloseBusy(false);
    }).then((fn) => {
      unlistenClose = fn;
    });

    void listen("recording:focus-request", () => {
      setScreen("home");
    }).then((fn) => {
      unlistenFocus = fn;
    });

    return () => {
      unlistenClose?.();
      unlistenFocus?.();
    };
  }, [setScreen]);

  const dismissClose = useCallback(() => {
    if (closeBusy) return;
    setClosePhase(null);
  }, [closeBusy]);

  async function stopAndSave() {
    setCloseBusy(true);
    try {
      const stopped = await recordStop();
      try {
        await hideRecorderWidget();
      } catch {
        // Widget hide is best-effort before the saved-path dialog.
      }
      try {
        await hideRecordingTray();
      } catch {
        // Tray hide is best-effort before the saved-path dialog.
      }
      // Panel may be unmounted (settings); clear recording busy for the
      // progress bar / updateGate without waiting for a remount sync.
      setWorkspaceBusy(false);
      setClosePhase({ kind: "saved", path: stopped.path });
    } catch (err) {
      const appErr = err as AppError;
      setClosePhase({
        kind: "error",
        message: friendlyErrorMessage(appErr),
        code: errorTitle(appErr),
      });
    } finally {
      setCloseBusy(false);
    }
  }

  async function acknowledgeSavedAndExit() {
    try {
      await exit(0);
    } catch {
      // Exit is best-effort; process may already be leaving.
    }
  }

  function openAbout() {
    setSettingsTab("about");
    setScreen("settings");
  }

  return (
    <div className="app-shell">
      <header className="app-header">
        <button
          type="button"
          className="brand"
          onClick={() => setScreen("home")}
        >
          Meetphant
          {screen === "home" && titleSuffix ? (
            <span className="brand-meta">· {titleSuffix}</span>
          ) : null}
        </button>
        <div className="app-header-actions">
          {screen === "settings" ? (
            <IconButton label="返回工作区" onClick={() => setScreen("home")}>
              <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
                <path
                  d="M15 6 9 12l6 6"
                  stroke="currentColor"
                  strokeWidth="1.7"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              </svg>
            </IconButton>
          ) : (
            <IconButton
              label="设置"
              onClick={() => {
                setSettingsTab("credentials");
                setScreen("settings");
              }}
            >
              <SettingsGearWithBadge />
            </IconButton>
          )}
        </div>
        {workspaceBusy && (
          <div className="app-progress" role="progressbar" aria-label="工作进行中">
            <div className="app-progress-bar" />
          </div>
        )}
      </header>
      <UpdateBanner onOpenAbout={openAbout} />
      <main className="app-main">
        {screen === "settings" ? (
          <SettingsPage key={settingsTab} initialTab={settingsTab} />
        ) : (
          <HomePage
            onOpenSettings={() => {
              setSettingsTab("credentials");
              setScreen("settings");
            }}
            onTranscribingChange={setWorkspaceBusy}
            onActiveTitleChange={setTitleSuffix}
          />
        )}
      </main>
      {closePhase ? (
        <CloseRecordingDialog
          phase={closePhase}
          busy={closeBusy}
          onStopAndSave={() => void stopAndSave()}
          onContinue={dismissClose}
          onCancel={dismissClose}
          onAcknowledgeSaved={() => void acknowledgeSavedAndExit()}
          onDismissError={dismissClose}
        />
      ) : null}
    </div>
  );
}

export function AppShell() {
  const [screen, setScreen] = useState<Screen>("home");
  const [settingsTab, setSettingsTab] = useState<SettingsTab>("credentials");
  const [workspaceBusy, setWorkspaceBusy] = useState(false);
  const [titleSuffix, setTitleSuffix] = useState<string | null>(null);

  return (
    <AppUpdateProvider appBusy={workspaceBusy}>
      <AppShellChrome
        screen={screen}
        setScreen={setScreen}
        settingsTab={settingsTab}
        setSettingsTab={setSettingsTab}
        workspaceBusy={workspaceBusy}
        setWorkspaceBusy={setWorkspaceBusy}
        titleSuffix={titleSuffix}
        setTitleSuffix={setTitleSuffix}
      />
    </AppUpdateProvider>
  );
}
