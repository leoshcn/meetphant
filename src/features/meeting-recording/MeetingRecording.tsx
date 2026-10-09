import { useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  jobsStartTranscription,
  meetingsAttachSource,
  meetingsCreateFromFile,
  recordListInputDevices,
  recordStart,
  recordStatus,
  recordStop,
  type AppError,
  type InputDevice,
  type Meeting,
} from "../../ipc";
import {
  errorTitle,
  formatRecordingElapsed,
  friendlyErrorMessage,
  hideMainToTray,
  hideRecorderWidget,
  restoreMainFromTray,
  showRecorderWidget,
} from "../../shared/lib";
import { Button } from "../../shared/ui";
import { RecordingWaveform } from "./RecordingWaveform";
import styles from "./MeetingRecording.module.css";

type Props = {
  /** When set, attach audio to this draft instead of creating a new meeting. */
  draftMeetingId?: string | null;
  onOpenSettings?: () => void;
  onMeetingCreated?: (meeting: Meeting, jobId?: string) => void;
  onBusyChange?: (busy: boolean) => void;
  onTitleResolved?: (title: string | null) => void;
  onReset?: () => void;
};

const STATUS_SYNC_MS = 1000;

export function MeetingRecordingPanel({
  draftMeetingId = null,
  onOpenSettings,
  onMeetingCreated,
  onBusyChange,
  onTitleResolved,
  onReset,
}: Props) {
  const [devices, setDevices] = useState<InputDevice[]>([]);
  const [deviceId, setDeviceId] = useState<string>("");
  const [recording, setRecording] = useState(false);
  const [deviceName, setDeviceName] = useState<string | null>(null);
  const [outputDeviceName, setOutputDeviceName] = useState<string | null>(null);
  const [elapsedMs, setElapsedMs] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [errorCode, setErrorCode] = useState<string | undefined>();
  const startedAtRef = useRef<number | null>(null);
  const tickRef = useRef<number | null>(null);
  const busyRef = useRef(false);
  const recordingRef = useRef(false);
  const [statusReady, setStatusReady] = useState(false);

  useEffect(() => {
    busyRef.current = busy;
  }, [busy]);

  useEffect(() => {
    recordingRef.current = recording;
  }, [recording]);

  // Gate onBusyChange until the first record_status sync so remounting after
  // settings does not briefly report idle and flicker workspaceBusy / updateGate.
  useEffect(() => {
    if (!statusReady) return;
    onBusyChange?.(busy || recording);
  }, [busy, recording, onBusyChange, statusReady]);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const listed = await recordListInputDevices();
        if (cancelled) return;
        setDevices(listed.devices);
        const initial =
          listed.default_id ?? listed.devices[0]?.id ?? "";
        setDeviceId(initial);
      } catch (err) {
        if (!cancelled) {
          const appErr = err as AppError;
          setError(friendlyErrorMessage(appErr));
          setErrorCode(errorTitle(appErr));
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    return () => {
      if (tickRef.current !== null) {
        window.clearInterval(tickRef.current);
      }
    };
  }, []);

  function clearTick() {
    if (tickRef.current !== null) {
      window.clearInterval(tickRef.current);
      tickRef.current = null;
    }
  }

  function startTickFrom(startedAtMs: number) {
    clearTick();
    startedAtRef.current = startedAtMs;
    setElapsedMs(Math.max(0, Date.now() - startedAtMs));
    tickRef.current = window.setInterval(() => {
      if (startedAtRef.current !== null) {
        setElapsedMs(Date.now() - startedAtRef.current);
      }
    }, 250);
  }

  function applyIdleUi() {
    clearTick();
    startedAtRef.current = null;
    setElapsedMs(0);
    setRecording(false);
    setDeviceName(null);
    setOutputDeviceName(null);
  }

  function applyRecordingStatus(status: {
    started_at: string | null;
    device_name: string | null;
    output_device_name: string | null;
  }) {
    const parsed = status.started_at ? Date.parse(status.started_at) : NaN;
    if (Number.isFinite(parsed)) {
      if (startedAtRef.current !== parsed) {
        startTickFrom(parsed);
      }
    }
    setDeviceName(status.device_name);
    setOutputDeviceName(status.output_device_name);
    setRecording(true);
  }

  useEffect(() => {
    let cancelled = false;

    async function syncStatus() {
      if (busyRef.current) {
        // Still unlock onBusyChange so a slow first poll cannot leave busy
        // reporting gated while the user already started an action.
        if (!cancelled) setStatusReady(true);
        return;
      }
      try {
        const status = await recordStatus();
        if (cancelled) return;
        if (status.state === "recording") {
          applyRecordingStatus(status);
        } else if (!busyRef.current && recordingRef.current) {
          // External stop (close-intercept path, etc.) — converge local UI.
          applyIdleUi();
          void hideRecorderWidget().catch(() => {
            // Ignore hide races during exit.
          });
          void restoreMainFromTray().catch(() => {
            // Ignore tray races during exit.
          });
        }
        setStatusReady(true);
      } catch {
        // Ignore transient status failures; still unblock busy reporting so
        // start/stop actions are not stuck gated behind a failed first poll.
        if (!cancelled) setStatusReady(true);
      }
    }

    void syncStatus();
    const id = window.setInterval(() => {
      void syncStatus();
    }, STATUS_SYNC_MS);

    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
    // Intentionally mount-only: sync polls independently of local recording flag.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function resolveMeetingFromPath(path: string): Promise<Meeting> {
    if (draftMeetingId) {
      return meetingsAttachSource(draftMeetingId, path);
    }
    return meetingsCreateFromFile(path);
  }

  async function beginRecording() {
    setError(null);
    setErrorCode(undefined);
    setBusy(true);
    onReset?.();
    try {
      const started = await recordStart(deviceId || null);
      setDeviceName(started.device_name);
      setOutputDeviceName(started.output_device_name);
      const status = await recordStatus();
      if (status.state === "recording" && status.started_at) {
        applyRecordingStatus(status);
      } else {
        setRecording(true);
        startTickFrom(Date.now());
      }
      try {
        await showRecorderWidget();
      } catch {
        // Widget is best-effort; recording continues without it.
      }
      try {
        await hideMainToTray();
      } catch {
        // Tray hide is best-effort; recording continues with main visible.
      }
    } catch (err) {
      const appErr = err as AppError;
      setError(friendlyErrorMessage(appErr));
      setErrorCode(errorTitle(appErr));
    } finally {
      setBusy(false);
    }
  }

  async function endRecording() {
    setError(null);
    setErrorCode(undefined);
    setBusy(true);
    clearTick();
    try {
      // Restore main before stop so the user sees export / transcription UI.
      try {
        await restoreMainFromTray();
      } catch {
        // Best-effort; stop proceeds even if tray/main restore fails.
      }
      const stopped = await recordStop();
      applyIdleUi();
      try {
        await hideRecorderWidget();
      } catch {
        // Ignore hide failures after stop.
      }
      const created = await resolveMeetingFromPath(stopped.path);
      onTitleResolved?.(created.title);
      const started = await jobsStartTranscription(created.id);
      onMeetingCreated?.(created, started.id);
    } catch (err) {
      const appErr = err as AppError;
      setError(friendlyErrorMessage(appErr));
      setErrorCode(errorTitle(appErr));
      applyIdleUi();
      try {
        await hideRecorderWidget();
      } catch {
        // Ignore.
      }
      try {
        await restoreMainFromTray();
      } catch {
        // Ignore.
      }
    } finally {
      setBusy(false);
    }
  }

  async function importAndTranscribe() {
    setError(null);
    setErrorCode(undefined);
    setBusy(true);
    onReset?.();
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [
          {
            name: "Audio",
            extensions: ["wav", "mp3", "m4a", "flac", "ogg", "aac"],
          },
        ],
      });
      if (selected === null) {
        setBusy(false);
        return;
      }
      const path = Array.isArray(selected) ? selected[0] : selected;
      const created = await resolveMeetingFromPath(path);
      onTitleResolved?.(created.title);
      const started = await jobsStartTranscription(created.id);
      onMeetingCreated?.(created, started.id);
    } catch (err) {
      const appErr = err as AppError;
      setError(friendlyErrorMessage(appErr));
      setErrorCode(errorTitle(appErr));
    } finally {
      setBusy(false);
    }
  }

  const needsSettingsHint =
    errorCode === "TOS_NOT_CONFIGURED" ||
    errorCode === "RECORD_DEVICE_ERROR" ||
    errorCode === "IO_ERROR";

  if (recording) {
    return (
      <section className={styles.stage}>
        <p className={styles.brand}>Meetphant</p>
        <h1 className={styles.stageTitle}>正在录音</h1>
        <p className={styles.timer} aria-live="polite">
          {formatRecordingElapsed(elapsedMs)}
        </p>
        <RecordingWaveform active={recording} />
        {deviceName && (
          <p className={styles.deviceLive}>麦克风：{deviceName}</p>
        )}
        {outputDeviceName && (
          <p className={styles.deviceLive}>系统声音：{outputDeviceName}</p>
        )}
        <div className={styles.stageActions}>
          <Button
            variant="primary"
            onClick={() => void endRecording()}
            disabled={busy}
          >
            {busy ? "正在导出…" : "停止并转写"}
          </Button>
        </div>
        {error && (
          <div className={styles.errorBlock} role="alert" title={errorCode}>
            <p className={styles.error}>{error}</p>
          </div>
        )}
      </section>
    );
  }

  return (
    <section className={styles.stage}>
      <p className={styles.brand}>Meetphant</p>
      <h1 className={styles.stageTitle}>把会议录音变成可带走的纪要</h1>
      <p className={styles.stageLead}>
        开始录音将同时捕获麦克风与系统扬声器（会议对方声音），或导入本地音频，自动转写并整理纪要。
      </p>

      <label className={styles.deviceField}>
        麦克风
        <select
          value={deviceId}
          onChange={(e) => setDeviceId(e.target.value)}
          disabled={busy || devices.length === 0}
        >
          {devices.length === 0 ? (
            <option value="">无可用设备</option>
          ) : (
            devices.map((d) => (
              <option key={d.id} value={d.id}>
                {d.name}
                {d.is_default ? "（默认）" : ""}
              </option>
            ))
          )}
        </select>
      </label>

      <div className={styles.stageActions}>
        <Button
          variant="primary"
          onClick={() => void beginRecording()}
          disabled={busy || !deviceId}
        >
          开始录音
        </Button>
        <Button
          variant="secondary"
          onClick={() => void importAndTranscribe()}
          disabled={busy || recording}
        >
          导入音频并转写
        </Button>
      </div>

      <p className={styles.stageHint}>
        系统声音来自默认播放设备 · 转写需配置豆包 API Key 与火山 TOS
      </p>

      {error && (
        <div className={styles.errorBlock} role="alert" title={errorCode}>
          <p className={styles.error}>{error}</p>
          {needsSettingsHint && onOpenSettings && (
            <Button variant="secondary" onClick={onOpenSettings}>
              去设置
            </Button>
          )}
        </div>
      )}
    </section>
  );
}
