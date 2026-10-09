import { useEffect, useRef, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { TranscriptionImportPanel } from "../../features/transcription-import";
import { MeetingRecordingPanel } from "../../features/meeting-recording";
import { MeetingSummaryPanel } from "../../features/meeting-summary";
import { MeetingSidebar } from "../../features/meeting-sidebar";
import type { Meeting } from "../../ipc";
import styles from "./HomePage.module.css";

const WORKSPACE_SPLIT_MIN = 960;

type WorkspaceTab = "transcript" | "summary";

type Props = {
  onOpenSettings: () => void;
  onTranscribingChange?: (busy: boolean) => void;
  onActiveTitleChange?: (title: string | null) => void;
};

function defaultWorkspaceTab(transcribing: boolean): WorkspaceTab {
  return transcribing ? "transcript" : "summary";
}

function useWideWorkspace(minWidth: number): boolean {
  const [wide, setWide] = useState(() => {
    if (typeof window === "undefined") return true;
    return window.matchMedia(`(min-width: ${minWidth}px)`).matches;
  });

  useEffect(() => {
    const mq = window.matchMedia(`(min-width: ${minWidth}px)`);
    const onChange = () => setWide(mq.matches);
    onChange();
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, [minWidth]);

  return wide;
}

function WorkspacePanes({
  wide,
  workspaceTab,
  meetingId,
  bootstrapJobId,
  summaryEpoch,
  hasTranscript,
  onOpenSettings,
  onBootstrapJobConsumed,
  onMeetingCreated,
  onTranscriptReady,
  onSpeakersUpdated,
  onBusyChange,
  onTitleResolved,
  onReset,
}: {
  wide: boolean;
  workspaceTab: WorkspaceTab;
  meetingId: string;
  bootstrapJobId: string | null;
  summaryEpoch: number;
  hasTranscript: boolean;
  onOpenSettings: () => void;
  onBootstrapJobConsumed: () => void;
  onMeetingCreated: (meeting: Meeting) => void;
  onTranscriptReady: (id: string) => void;
  onSpeakersUpdated: () => void;
  onBusyChange: (busy: boolean) => void;
  onTitleResolved: (title: string | null) => void;
  onReset: () => void;
}) {
  const transcriptPane = (
    <section
      className={styles.pane}
      aria-label="转写"
      role={wide ? undefined : "tabpanel"}
      id={wide ? undefined : "workspace-panel-transcript"}
      aria-labelledby={wide ? undefined : "workspace-tab-transcript"}
      hidden={wide ? undefined : workspaceTab !== "transcript"}
    >
      <header className={styles.paneHeader}>
        <h2>转写</h2>
      </header>
      <div className={styles.paneBody}>
        <TranscriptionImportPanel
          meetingId={meetingId}
          layout="pane"
          bootstrapJobId={bootstrapJobId}
          onBootstrapJobConsumed={onBootstrapJobConsumed}
          onOpenSettings={onOpenSettings}
          onMeetingCreated={onMeetingCreated}
          onTranscriptReady={onTranscriptReady}
          onSpeakersUpdated={onSpeakersUpdated}
          onBusyChange={onBusyChange}
          onTitleResolved={onTitleResolved}
          onReset={onReset}
        />
      </div>
    </section>
  );

  const summaryPane = (
    <section
      className={styles.pane}
      aria-label="摘要"
      role={wide ? undefined : "tabpanel"}
      id={wide ? undefined : "workspace-panel-summary"}
      aria-labelledby={wide ? undefined : "workspace-tab-summary"}
      hidden={wide ? undefined : workspaceTab !== "summary"}
    >
      <header className={styles.paneHeader}>
        <h2>摘要</h2>
      </header>
      <div className={styles.paneBody}>
        <MeetingSummaryPanel
          meetingId={meetingId}
          summaryEpoch={summaryEpoch}
          ready={hasTranscript}
          onOpenSettings={onOpenSettings}
        />
      </div>
    </section>
  );

  if (wide) {
    return (
      <>
        {transcriptPane}
        {summaryPane}
      </>
    );
  }

  return (
    <div className={styles.tabPanels}>
      {transcriptPane}
      {summaryPane}
    </div>
  );
}

export function HomePage({
  onOpenSettings,
  onTranscribingChange,
  onActiveTitleChange,
}: Props) {
  const [activeMeetingId, setActiveMeetingId] = useState<string | null>(null);
  const [activeTitle, setActiveTitle] = useState<string | null>(null);
  const [activeSourcePath, setActiveSourcePath] = useState("");
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [listRefreshKey, setListRefreshKey] = useState(0);
  const [summaryEpoch, setSummaryEpoch] = useState(0);
  const [hasTranscript, setHasTranscript] = useState(false);
  const [transcribing, setTranscribing] = useState(false);
  const [bootstrapJobId, setBootstrapJobId] = useState<string | null>(null);
  const [workspaceTab, setWorkspaceTab] = useState<WorkspaceTab>("summary");
  const wasTranscribing = useRef(false);
  const wideWorkspace = useWideWorkspace(WORKSPACE_SPLIT_MIN);

  function bumpList() {
    setListRefreshKey((k) => k + 1);
  }

  useEffect(() => {
    onTranscribingChange?.(transcribing);
  }, [transcribing, onTranscribingChange]);

  useEffect(() => {
    onActiveTitleChange?.(activeMeetingId ? activeTitle : null);
  }, [activeMeetingId, activeTitle, onActiveTitleChange]);

  useEffect(() => {
    if (wasTranscribing.current && !transcribing && hasTranscript) {
      setWorkspaceTab((tab) => (tab === "transcript" ? "summary" : tab));
    }
    wasTranscribing.current = transcribing;
  }, [transcribing, hasTranscript]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen("recording:focus-request", () => {
      // Return to the recording stage (R6.2), not another meeting workspace.
      setActiveMeetingId(null);
      setActiveTitle(null);
      setActiveSourcePath("");
      setHasTranscript(false);
      setBootstrapJobId(null);
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  const isDraft =
    activeMeetingId !== null && activeSourcePath.trim().length === 0;
  const showWorkspace = activeMeetingId !== null && !isDraft;

  function resetWorkspaceTab(nextTranscribing: boolean) {
    setWorkspaceTab(defaultWorkspaceTab(nextTranscribing));
  }

  function handleMeetingCreated(meeting: Meeting, jobId?: string) {
    setActiveMeetingId(meeting.id);
    setActiveTitle(meeting.title);
    setActiveSourcePath(meeting.source_path);
    setHasTranscript(false);
    setBootstrapJobId(jobId ?? null);
    resetWorkspaceTab(Boolean(jobId) || transcribing);
    bumpList();
  }

  let workspaceBody: ReactNode;
  if (!showWorkspace || !activeMeetingId) {
    workspaceBody = (
      <div className={styles.stageBody}>
        <MeetingRecordingPanel
          draftMeetingId={isDraft ? activeMeetingId : null}
          onOpenSettings={onOpenSettings}
          onMeetingCreated={handleMeetingCreated}
          onBusyChange={setTranscribing}
          onTitleResolved={(title) => setActiveTitle(title)}
          onReset={() => {
            setHasTranscript(false);
          }}
        />
      </div>
    );
  } else {
    workspaceBody = (
      <>
        {!wideWorkspace && (
          <div
            className={styles.workspaceTabs}
            role="tablist"
            aria-label="工作区"
          >
            <button
              type="button"
              role="tab"
              id="workspace-tab-transcript"
              aria-selected={workspaceTab === "transcript"}
              aria-controls="workspace-panel-transcript"
              className={
                workspaceTab === "transcript"
                  ? styles.workspaceTabActive
                  : styles.workspaceTab
              }
              onClick={() => setWorkspaceTab("transcript")}
            >
              转写
            </button>
            <button
              type="button"
              role="tab"
              id="workspace-tab-summary"
              aria-selected={workspaceTab === "summary"}
              aria-controls="workspace-panel-summary"
              className={
                workspaceTab === "summary"
                  ? styles.workspaceTabActive
                  : styles.workspaceTab
              }
              onClick={() => setWorkspaceTab("summary")}
            >
              摘要
            </button>
          </div>
        )}
        <WorkspacePanes
          wide={wideWorkspace}
          workspaceTab={workspaceTab}
          meetingId={activeMeetingId}
          bootstrapJobId={bootstrapJobId}
          summaryEpoch={summaryEpoch}
          hasTranscript={hasTranscript}
          onOpenSettings={onOpenSettings}
          onBootstrapJobConsumed={() => setBootstrapJobId(null)}
          onMeetingCreated={(meeting) => handleMeetingCreated(meeting)}
          onTranscriptReady={(id) => {
            setActiveMeetingId(id);
            setHasTranscript(true);
            bumpList();
          }}
          onSpeakersUpdated={() => {
            setSummaryEpoch((n) => n + 1);
            setHasTranscript(true);
          }}
          onBusyChange={setTranscribing}
          onTitleResolved={(title) => setActiveTitle(title)}
          onReset={() => {
            setHasTranscript(false);
          }}
        />
      </>
    );
  }

  return (
    <div className={styles.layout}>
      <MeetingSidebar
        activeMeetingId={activeMeetingId}
        refreshKey={listRefreshKey}
        collapsed={sidebarCollapsed}
        onToggleCollapsed={() => setSidebarCollapsed((c) => !c)}
        onSelect={(id, meeting) => {
          setActiveMeetingId(id);
          setActiveTitle(meeting?.title ?? null);
          setActiveSourcePath(meeting?.source_path ?? "");
          setHasTranscript(false);
          setBootstrapJobId(null);
          resetWorkspaceTab(false);
        }}
        onNewProject={(meeting) => {
          setActiveMeetingId(meeting.id);
          setActiveTitle(meeting.title);
          setActiveSourcePath(meeting.source_path);
          setHasTranscript(false);
          setTranscribing(false);
          setBootstrapJobId(null);
          resetWorkspaceTab(false);
        }}
        onDeleted={(id) => {
          if (activeMeetingId === id) {
            setActiveMeetingId(null);
            setActiveTitle(null);
            setActiveSourcePath("");
            setHasTranscript(false);
            setTranscribing(false);
          }
          bumpList();
        }}
        onRenamed={(meeting) => {
          if (meeting.id === activeMeetingId) {
            setActiveTitle(meeting.title);
            setActiveSourcePath(meeting.source_path);
          }
          bumpList();
        }}
      />
      <div className={styles.workspace}>
        <div
          className={
            showWorkspace
              ? `${wideWorkspace ? styles.split : styles.tabsLayout} meetphant-fade-up`
              : `${styles.emptyStage} meetphant-fade-up`
          }
        >
          {workspaceBody}
        </div>
      </div>
    </div>
  );
}
