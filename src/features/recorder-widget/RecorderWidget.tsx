import { useEffect, useRef, useState } from "react";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { emitTo } from "@tauri-apps/api/event";
import { getCurrentWindow, Window } from "@tauri-apps/api/window";
import { recordStatus } from "../../ipc";
import {
  COLLAPSED_SIZE,
  EXPANDED_SIZE,
  formatRecordingElapsed,
  persistWidgetOuterPosition,
} from "../../shared/lib/recorderWidget";
import styles from "./RecorderWidget.module.css";

const POLL_EXPANDED_MS = 60;
const POLL_COLLAPSED_MS = 1000;
const MOVE_SAVE_DEBOUNCE_MS = 200;
/** Movement past this (px) starts a window drag instead of expand-on-click. */
const COLLAPSED_DRAG_THRESHOLD_PX = 4;

export function RecorderWidget() {
  const [collapsed, setCollapsed] = useState(false);
  const [elapsedMs, setElapsedMs] = useState(0);
  const [micLevel, setMicLevel] = useState(0);
  const [systemLevel, setSystemLevel] = useState(0);
  const [reducedMotion, setReducedMotion] = useState(false);
  const startedAtMsRef = useRef<number | null>(null);
  const tickRef = useRef<number | null>(null);
  const pollRef = useRef<number | null>(null);
  const moveSaveTimerRef = useRef<number | null>(null);

  useEffect(() => {
    const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
    const sync = () => setReducedMotion(mq.matches);
    sync();
    mq.addEventListener("change", sync);
    return () => mq.removeEventListener("change", sync);
  }, []);

  useEffect(() => {
    const widget = getCurrentWindow();
    let cancelled = false;

    function clearTick() {
      if (tickRef.current !== null) {
        window.clearInterval(tickRef.current);
        tickRef.current = null;
      }
    }

    function startTick(startedAtMs: number) {
      startedAtMsRef.current = startedAtMs;
      clearTick();
      const update = () => {
        if (startedAtMsRef.current !== null) {
          setElapsedMs(Date.now() - startedAtMsRef.current);
        }
      };
      update();
      tickRef.current = window.setInterval(update, 250);
    }

    // Collapsed toggles remount this effect; keep the elapsed tick alive.
    if (startedAtMsRef.current !== null) {
      startTick(startedAtMsRef.current);
    }

    async function hideSelf() {
      clearTick();
      startedAtMsRef.current = null;
      setElapsedMs(0);
      setMicLevel(0);
      setSystemLevel(0);
      // New recording sessions always open expanded (showRecorderWidget sets
      // EXPANDED_SIZE); clear collapsed so a stale React state cannot leave a
      // collapsed UI inside an expanded transparent hit area (R3.2 / R3.3).
      setCollapsed(false);
      try {
        await widget.hide();
      } catch {
        // Ignore hide races during app exit.
      }
    }

    async function pollOnce() {
      try {
        const status = await recordStatus();
        if (cancelled) return;
        if (status.state !== "recording") {
          await hideSelf();
          return;
        }
        if (status.started_at) {
          const parsed = Date.parse(status.started_at);
          if (Number.isFinite(parsed) && startedAtMsRef.current !== parsed) {
            // started_at change ⇒ new session (or first observe). Reset fold so
            // a hide() from main that skipped hideSelf cannot leave collapsed
            // UI in an expanded window on the next recording.
            startTick(parsed);
            if (collapsed) {
              setCollapsed(false);
            }
          }
        }
        if (!collapsed) {
          setMicLevel(clamp01(status.mic_level));
          setSystemLevel(clamp01(status.system_level));
        }
      } catch {
        // Keep last frame if a poll fails mid-recording.
      }
    }

    void pollOnce();
    pollRef.current = window.setInterval(
      () => {
        void pollOnce();
      },
      collapsed ? POLL_COLLAPSED_MS : POLL_EXPANDED_MS,
    );

    return () => {
      cancelled = true;
      clearTick();
      if (pollRef.current !== null) {
        window.clearInterval(pollRef.current);
        pollRef.current = null;
      }
    };
  }, [collapsed]);

  useEffect(() => {
    const widget = getCurrentWindow();
    let unlisten: (() => void) | undefined;

    void widget
      .onMoved(() => {
        if (moveSaveTimerRef.current !== null) {
          window.clearTimeout(moveSaveTimerRef.current);
        }
        moveSaveTimerRef.current = window.setTimeout(() => {
          void persistWidgetOuterPosition(widget);
        }, MOVE_SAVE_DEBOUNCE_MS);
      })
      .then((fn) => {
        unlisten = fn;
      });

    return () => {
      unlisten?.();
      if (moveSaveTimerRef.current !== null) {
        window.clearTimeout(moveSaveTimerRef.current);
      }
    };
  }, []);

  async function setCollapsedMode(next: boolean) {
    const widget = getCurrentWindow();
    const size = next ? COLLAPSED_SIZE : EXPANDED_SIZE;
    await widget.setSize(new LogicalSize(size.width, size.height));
    setCollapsed(next);
  }

  async function openMeetphant() {
    const main = await Window.getByLabel("main");
    if (!main) return;
    try {
      await main.unminimize();
      await main.show();
      await main.setFocus();
    } catch {
      // Best-effort focus.
    }
    try {
      await emitTo("main", "recording:focus-request");
    } catch {
      // Event delivery is best-effort.
    }
  }

  /**
   * Collapsed pill: click expands; drag past a small threshold moves the window.
   * `data-tauri-drag-region` alone would steal clicks, so we startDragging manually.
   */
  function onCollapsedPointerDown(event: React.PointerEvent<HTMLDivElement>) {
    if (event.button !== 0) return;
    event.preventDefault();
    const startX = event.clientX;
    const startY = event.clientY;
    let dragging = false;

    const onMove = (ev: PointerEvent) => {
      if (dragging) return;
      if (
        Math.hypot(ev.clientX - startX, ev.clientY - startY) <
        COLLAPSED_DRAG_THRESHOLD_PX
      ) {
        return;
      }
      dragging = true;
      void getCurrentWindow()
        .startDragging()
        .catch(() => {
          // Drag may fail if the pointer was released mid-call.
        });
    };

    const onUp = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
      if (!dragging) {
        void setCollapsedMode(false);
      }
    };

    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
  }

  if (collapsed) {
    return (
      <div
        className={`${styles.pill} ${styles.collapsed}`}
        role="button"
        tabIndex={0}
        aria-label="展开录音悬浮窗，拖动可移动"
        onPointerDown={onCollapsedPointerDown}
        onKeyDown={(event) => {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            void setCollapsedMode(false);
          }
        }}
      >
        <span
          className={`${styles.dot} ${reducedMotion ? styles.dotStatic : ""}`}
          aria-hidden="true"
        />
      </div>
    );
  }

  const timerLabel = formatRecordingElapsed(elapsedMs);

  return (
    <div className={styles.pill} data-tauri-drag-region>
      <span
        className={`${styles.dot} ${reducedMotion ? styles.dotStatic : ""}`}
        aria-hidden="true"
      />
      <p className={styles.timer} aria-live="polite" aria-label={`已录 ${timerLabel}`}>
        {timerLabel}
      </p>
      <div className={styles.meters} aria-hidden="true">
        <div className={styles.meterTrack}>
          <div
            className={`${styles.meterFill} ${reducedMotion ? styles.meterFillStatic : ""}`}
            style={{ transform: `scaleX(${micLevel})` }}
          />
        </div>
        <div className={styles.meterTrack}>
          <div
            className={`${styles.meterFill} ${reducedMotion ? styles.meterFillStatic : ""}`}
            style={{ transform: `scaleX(${systemLevel})` }}
          />
        </div>
      </div>
      <div className={styles.actions}>
        <button
          type="button"
          className={styles.action}
          onClick={() => void openMeetphant()}
        >
          打开 Meetphant
        </button>
        <button
          type="button"
          className={styles.action}
          onClick={() => void setCollapsedMode(true)}
        >
          折叠
        </button>
      </div>
    </div>
  );
}

function clamp01(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.min(1, Math.max(0, value));
}
