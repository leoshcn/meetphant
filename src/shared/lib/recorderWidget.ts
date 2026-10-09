import { LogicalPosition, LogicalSize } from "@tauri-apps/api/dpi";
import {
  availableMonitors,
  primaryMonitor,
  Window,
  type Monitor,
} from "@tauri-apps/api/window";

export const RECORDER_WIDGET_LABEL = "recorder-widget";
export const RECORDER_WIDGET_POSITION_KEY = "meetphant.recorderWidget.position";

export const EXPANDED_SIZE = { width: 360, height: 52 } as const;
export const COLLAPSED_SIZE = { width: 36, height: 36 } as const;

export type WidgetPosition = {
  x: number;
  y: number;
};

function isFiniteNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

export function loadStoredPosition(): WidgetPosition | null {
  try {
    const raw = localStorage.getItem(RECORDER_WIDGET_POSITION_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as { x?: unknown; y?: unknown };
    if (!isFiniteNumber(parsed.x) || !isFiniteNumber(parsed.y)) return null;
    return { x: parsed.x, y: parsed.y };
  } catch {
    return null;
  }
}

export function saveWidgetPosition(position: WidgetPosition): void {
  localStorage.setItem(
    RECORDER_WIDGET_POSITION_KEY,
    JSON.stringify({ x: position.x, y: position.y }),
  );
}

export function clearWidgetPosition(): void {
  localStorage.removeItem(RECORDER_WIDGET_POSITION_KEY);
}

function rectsIntersect(
  ax: number,
  ay: number,
  aw: number,
  ah: number,
  bx: number,
  by: number,
  bw: number,
  bh: number,
): boolean {
  return ax < bx + bw && ax + aw > bx && ay < by + bh && ay + ah > by;
}

/** True when the widget rect intersects any monitor's visible area. */
export function positionIntersectsAnyMonitor(
  position: WidgetPosition,
  size: { width: number; height: number },
  monitors: Monitor[],
): boolean {
  for (const monitor of monitors) {
    const scale = monitor.scaleFactor;
    const work = monitor.workArea;
    const workX = work.position.toLogical(scale).x;
    const workY = work.position.toLogical(scale).y;
    const workW = work.size.toLogical(scale).width;
    const workH = work.size.toLogical(scale).height;
    if (
      rectsIntersect(
        position.x,
        position.y,
        size.width,
        size.height,
        workX,
        workY,
        workW,
        workH,
      )
    ) {
      return true;
    }
  }
  return false;
}

export async function defaultWidgetPosition(
  size: { width: number; height: number } = EXPANDED_SIZE,
): Promise<WidgetPosition> {
  const monitor = (await primaryMonitor()) ?? (await availableMonitors())[0];
  if (!monitor) {
    return { x: 80, y: 48 };
  }
  const scale = monitor.scaleFactor;
  const work = monitor.workArea;
  const workPos = work.position.toLogical(scale);
  const workSize = work.size.toLogical(scale);
  return {
    x: workPos.x + Math.max(0, (workSize.width - size.width) / 2),
    y: workPos.y + Math.min(48, Math.max(16, workSize.height * 0.04)),
  };
}

export async function resolveWidgetPosition(
  size: { width: number; height: number } = EXPANDED_SIZE,
): Promise<WidgetPosition> {
  const stored = loadStoredPosition();
  const monitors = await availableMonitors();
  if (stored && positionIntersectsAnyMonitor(stored, size, monitors)) {
    return stored;
  }
  const fallback = await defaultWidgetPosition(size);
  saveWidgetPosition(fallback);
  return fallback;
}

export async function getRecorderWidget(): Promise<Window | null> {
  return Window.getByLabel(RECORDER_WIDGET_LABEL);
}

export async function showRecorderWidget(): Promise<void> {
  const widget = await getRecorderWidget();
  if (!widget) return;
  const position = await resolveWidgetPosition(EXPANDED_SIZE);
  await widget.setSize(
    new LogicalSize(EXPANDED_SIZE.width, EXPANDED_SIZE.height),
  );
  await widget.setPosition(new LogicalPosition(position.x, position.y));
  await widget.show();
}

export async function hideRecorderWidget(): Promise<void> {
  const widget = await getRecorderWidget();
  if (!widget) return;
  await widget.hide();
}

export async function persistWidgetOuterPosition(widget: Window): Promise<void> {
  const outer = await widget.outerPosition();
  const factor = await widget.scaleFactor();
  const logical = outer.toLogical(factor);
  saveWidgetPosition({ x: logical.x, y: logical.y });
}

export function formatRecordingElapsed(ms: number): string {
  const totalSec = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(totalSec / 3600);
  const m = Math.floor((totalSec % 3600) / 60);
  const s = totalSec % 60;
  if (h > 0) {
    return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
  }
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}
