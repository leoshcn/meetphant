import { useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useTheme } from "../../app/ThemeProvider";
import {
  jobsGet,
  jobsStartTranscription,
  meetingsCreateFromFile,
  meetingsGet,
  meetingsGetTranscript,
  meetingsUpdateSpeakers,
  type AppError,
  type Job,
  type Meeting,
  type Transcript,
} from "../../ipc";
import { errorTitle, friendlyErrorMessage } from "../../shared/lib";
import { Button } from "../../shared/ui";
import styles from "./TranscriptionImport.module.css";

const POLL_MS = 1000;

const SPEAKER_COLORS_LIGHT = [
  "#1F5C57",
  "#3D5A80",
  "#8B5E3C",
  "#5C4A7A",
  "#2F6B3A",
  "#A65D3F",
];

/** Brighter palette for dark paper — ≥3:1 vs typical panel/paper. */
const SPEAKER_COLORS_DARK = [
  "#5FB8AE",
  "#7FA8D4",
  "#D4A574",
  "#B39BD4",
  "#7DBA88",
  "#E09A6E",
];

function speakerColors(theme: "light" | "dark"): string[] {
  return theme === "dark" ? SPEAKER_COLORS_DARK : SPEAKER_COLORS_LIGHT;
}

function speakerColor(
  speakerId: string,
  orderedIds: string[],
  theme: "light" | "dark",
): string {
  const index = Math.max(0, orderedIds.indexOf(speakerId));
  const palette = speakerColors(theme);
  return palette[index % palette.length];
}

type Props = {
  meetingId: string | null;
  layout?: "stage" | "pane";
  /** When set with a meetingId, start polling this transcription job. */
  bootstrapJobId?: string | null;
  onBootstrapJobConsumed?: () => void;
  onOpenSettings?: () => void;
  onMeetingCreated?: (meeting: Meeting) => void;
  onTranscriptReady?: (meetingId: string) => void;
  onSpeakersUpdated?: () => void;
  onBusyChange?: (busy: boolean) => void;
  onTitleResolved?: (title: string | null) => void;
  onReset?: () => void;
};

function uniqueSpeakerIds(transcript: Transcript): string[] {
  const seen = new Set<string>();
  const ids: string[] = [];
  for (const seg of transcript.segments) {
    if (!seen.has(seg.speaker_id)) {
      seen.add(seg.speaker_id);
      ids.push(seg.speaker_id);
    }
  }
  return ids;
}

function fileNameFromPath(path: string): string {
  const parts = path.split(/[/\\]/);
  return parts[parts.length - 1] || path;
}

export function TranscriptionImportPanel({
  meetingId,
  layout = "pane",
  bootstrapJobId = null,
  onBootstrapJobConsumed,
  onOpenSettings,
  onMeetingCreated,
  onTranscriptReady,
  onSpeakersUpdated,
  onBusyChange,
  onTitleResolved,
  onReset,
}: Props) {
  const { resolved: resolvedTheme } = useTheme();
  const [meeting, setMeeting] = useState<Meeting | null>(null);
  const [job, setJob] = useState<Job | null>(null);
  const [transcript, setTranscript] = useState<Transcript | null>(null);
  const [nameDraft, setNameDraft] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [errorCode, setErrorCode] = useState<string | undefined>();
  const [info, setInfo] = useState<string | null>(null);
  const pollRef = useRef<number | null>(null);
  const importingIdRef = useRef<string | null>(null);

  useEffect(() => {
    onBusyChange?.(busy);
  }, [busy, onBusyChange]);

  useEffect(() => {
    return () => {
      if (pollRef.current !== null) {
        window.clearInterval(pollRef.current);
      }
    };
  }, []);

  useEffect(() => {
    let cancelled = false;

    if (!meetingId) {
      stopPolling();
      setMeeting(null);
      setTranscript(null);
      setNameDraft({});
      setJob(null);
      setError(null);
      setErrorCode(undefined);
      setInfo(null);
      setBusy(false);
      return;
    }

    if (meetingId && importingIdRef.current === meetingId) {
      return;
    }
    importingIdRef.current = null;

    stopPolling();
    setJob(null);
    setError(null);
    setErrorCode(undefined);
    setInfo(null);
    setBusy(false);

    (async () => {
      try {
        const m = await meetingsGet(meetingId);
        if (cancelled) return;
        setMeeting(m);
        onTitleResolved?.(m.title);

        if (bootstrapJobId) {
          importingIdRef.current = meetingId;
          setBusy(true);
          setJob({
            id: bootstrapJobId,
            meeting_id: meetingId,
            kind: "transcription",
            status: "running",
            error_code: null,
            error_message: null,
            created_at: "",
            updated_at: "",
          });
          startPolling(bootstrapJobId, meetingId);
          onBootstrapJobConsumed?.();
          return;
        }

        try {
          const t = await meetingsGetTranscript(meetingId);
          if (cancelled) return;
          setTranscript(t);
          setNameDraft({ ...t.speaker_names });
          onTranscriptReady?.(meetingId);
        } catch (err) {
          const appErr = err as AppError;
          if (cancelled) return;
          setTranscript(null);
          setNameDraft({});
          if (appErr.code && appErr.code !== "NOT_FOUND") {
            setError(friendlyErrorMessage(appErr));
            setErrorCode(errorTitle(appErr));
          }
        }
      } catch (err) {
        const appErr = err as AppError;
        if (!cancelled) {
          setError(friendlyErrorMessage(appErr));
          setErrorCode(errorTitle(appErr));
        }
      }
    })();

    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [meetingId, bootstrapJobId]);

  function stopPolling() {
    if (pollRef.current !== null) {
      window.clearInterval(pollRef.current);
      pollRef.current = null;
    }
  }

  function startPolling(jobId: string, createdMeetingId: string) {
    stopPolling();
    pollRef.current = window.setInterval(async () => {
      try {
        const next = await jobsGet(jobId);
        setJob(next);
        if (next.status === "succeeded") {
          stopPolling();
          const t = await meetingsGetTranscript(createdMeetingId);
          setTranscript(t);
          setNameDraft({ ...t.speaker_names });
          setBusy(false);
          importingIdRef.current = null;
          onTranscriptReady?.(createdMeetingId);
        } else if (next.status === "failed") {
          stopPolling();
          const failed: AppError = {
            code: next.error_code ?? "ERROR",
            message: next.error_message ?? "Transcription failed",
          };
          setError(friendlyErrorMessage(failed));
          setErrorCode(errorTitle(failed));
          setBusy(false);
          importingIdRef.current = null;
        }
      } catch (err) {
        stopPolling();
        const appErr = err as AppError;
        setError(friendlyErrorMessage(appErr));
        setErrorCode(errorTitle(appErr));
        setBusy(false);
        importingIdRef.current = null;
      }
    }, POLL_MS);
  }

  async function importAndTranscribe() {
    setError(null);
    setErrorCode(undefined);
    setInfo(null);
    setTranscript(null);
    setNameDraft({});
    setJob(null);
    setMeeting(null);
    setBusy(true);
    stopPolling();
    importingIdRef.current = null;
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
      const created = await meetingsCreateFromFile(path);
      importingIdRef.current = created.id;
      setMeeting(created);
      onTitleResolved?.(created.title);
      onMeetingCreated?.(created);
      const started = await jobsStartTranscription(created.id);
      setJob(started);
      startPolling(started.id, created.id);
    } catch (err) {
      const appErr = err as AppError;
      setError(friendlyErrorMessage(appErr));
      setErrorCode(errorTitle(appErr));
      setBusy(false);
      importingIdRef.current = null;
    }
  }

  async function applySpeakerNames() {
    if (!meeting || !transcript || transcript.segments.length === 0) {
      return;
    }
    setError(null);
    setErrorCode(undefined);
    setInfo(null);
    try {
      const updated = await meetingsUpdateSpeakers(meeting.id, nameDraft);
      setTranscript(updated);
      setNameDraft({ ...updated.speaker_names });
      setInfo("已更新发言人名称。若已有摘要，请重新生成。");
      onSpeakersUpdated?.();
    } catch (err) {
      const appErr = err as AppError;
      setError(friendlyErrorMessage(appErr));
      setErrorCode(errorTitle(appErr));
    }
  }

  const speakerIds = transcript ? uniqueSpeakerIds(transcript) : [];
  const needsTosHint =
    errorCode === "TOS_NOT_CONFIGURED" || errorCode === "TOS_UPLOAD_ERROR";

  const importButton = (
    <Button
      variant="primary"
      onClick={() => void importAndTranscribe()}
      disabled={busy}
    >
      {busy ? "转写中…" : "导入音频并转写"}
    </Button>
  );

  const errorBlock =
    error && (
      <div className={styles.errorBlock} role="alert" title={errorCode}>
        <p className={styles.error}>{error}</p>
        {needsTosHint && onOpenSettings && (
          <Button variant="secondary" onClick={onOpenSettings}>
            去设置配置 TOS
          </Button>
        )}
      </div>
    );

  if (layout === "stage") {
    return (
      <section className={styles.stage}>
        <p className={styles.brand}>Meetphant</p>
        <h1 className={styles.stageTitle}>把会议录音变成可带走的纪要</h1>
        <p className={styles.stageLead}>
          导入本地音频，自动转写并整理要点、待办与决策。
        </p>
        <div className={styles.stageAction}>{importButton}</div>
        <p className={styles.stageHint}>
          豆包录音文件识别 2.0 · 需配置火山 TOS（上限 512 MiB）
        </p>
        {job && busy && (
          <p className={styles.status}>正在转写…（{job.status}）</p>
        )}
        {errorBlock}
      </section>
    );
  }

  return (
    <section className={styles.pane}>
      {!transcript && (
        <div className={styles.paneIntro}>
          {meeting ? (
            <>
              <p className={styles.meta}>
                <span className={styles.metaLabel}>文件</span>
                {fileNameFromPath(meeting.source_path)}
              </p>
              {busy || job?.status === "running" ? (
                <p className={styles.status}>正在转写，完成后会出现全文与发言人。</p>
              ) : (
                <>
                  <p className={styles.hint}>
                    此项目还没有转写结果。可重新导入音频，或等待进行中的任务完成。
                  </p>
                  {importButton}
                </>
              )}
            </>
          ) : (
            <>
              <p className={styles.hint}>选择音频开始转写。</p>
              {importButton}
            </>
          )}
        </div>
      )}

      {transcript !== null && (
        <>
          {meeting && (
            <p className={styles.meta}>
              <span className={styles.metaLabel}>文件</span>
              {fileNameFromPath(meeting.source_path)}
            </p>
          )}
          {speakerIds.length > 0 && (
            <div className={styles.speakers}>
              <div className={styles.speakersHead}>
                <h3>发言人</h3>
                <Button variant="secondary" onClick={() => void applySpeakerNames()}>
                  应用名称
                </Button>
              </div>
              <div className={styles.speakerList}>
                {speakerIds.map((id) => (
                  <label key={id} className={styles.speakerRow}>
                    <span
                      className={styles.speakerSwatch}
                      style={{ background: speakerColor(id, speakerIds, resolvedTheme) }}
                      aria-hidden
                    />
                    <span className={styles.speakerId}>ID {id}</span>
                    <input
                      value={nameDraft[id] ?? ""}
                      onChange={(e) =>
                        setNameDraft((prev) => ({
                          ...prev,
                          [id]: e.target.value,
                        }))
                      }
                      placeholder={`发言人 ${id}`}
                    />
                  </label>
                ))}
              </div>
            </div>
          )}
          <div className={styles.transcript}>
            {transcript.segments.length === 0 ? (
              <pre className={styles.transcriptFallback}>
                {transcript.text || "（空转写结果）"}
              </pre>
            ) : (
              transcript.segments.map((seg, index) => {
                const name =
                  nameDraft[seg.speaker_id] ||
                  transcript.speaker_names[seg.speaker_id] ||
                  `发言人 ${seg.speaker_id}`;
                const color = speakerColor(seg.speaker_id, speakerIds, resolvedTheme);
                return (
                  <article
                    key={`${seg.speaker_id}-${index}`}
                    className={styles.segment}
                    style={{ ["--rail" as string]: color }}
                  >
                    <header className={styles.segmentHead}>{name}</header>
                    <p className={styles.segmentText}>{seg.text}</p>
                  </article>
                );
              })
            )}
          </div>
          <div className={styles.reimport}>{importButton}</div>
        </>
      )}

      {info && <p className={styles.info}>{info}</p>}
      {errorBlock}
    </section>
  );
}
