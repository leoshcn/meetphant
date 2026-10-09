import type { ThemePreference } from "../../ipc";

export type ResolvedTheme = "light" | "dark";

const CACHE_KEY = "meetphant.theme_preference";

export function parseThemePreference(raw: unknown): ThemePreference {
  if (raw === "light" || raw === "dark" || raw === "system") return raw;
  return "system";
}

export function systemPrefersDark(): boolean {
  if (typeof window === "undefined" || !window.matchMedia) return false;
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

export function resolveTheme(preference: ThemePreference): ResolvedTheme {
  if (preference === "light") return "light";
  if (preference === "dark") return "dark";
  return systemPrefersDark() ? "dark" : "light";
}

export function applyResolvedTheme(resolved: ResolvedTheme): void {
  const root = document.documentElement;
  root.setAttribute("data-theme", resolved);
  // Transparent floating windows must not paint a solid color-scheme canvas
  // behind rounded CSS (shows as a rectangle on black/white desktops).
  if (root.classList.contains("recorder-widget-shell")) {
    root.style.colorScheme = "normal";
  } else {
    root.style.colorScheme = resolved;
  }
}

export function readCachedThemePreference(): ThemePreference | null {
  try {
    const raw = localStorage.getItem(CACHE_KEY);
    if (raw === null) return null;
    return parseThemePreference(raw);
  } catch {
    return null;
  }
}

export function writeCachedThemePreference(preference: ThemePreference): void {
  try {
    localStorage.setItem(CACHE_KEY, preference);
  } catch {
    // Non-authoritative cache; ignore quota / private mode failures.
  }
}

/** Apply last-known preference before Settings load to reduce FOUC. */
export function bootstrapThemeFromCache(): void {
  const cached = readCachedThemePreference();
  if (!cached) return;
  applyResolvedTheme(resolveTheme(cached));
}
