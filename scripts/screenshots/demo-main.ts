/**
 * Dev-only demo bootstrap: installs Tauri IPC/window mocks, then loads the
 * real app entry so the screenshots show genuine React components.
 *
 * URL params: ?scene=workspace|recording|hotwords|llm|home&theme=dark|light
 */
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { handle, type Scene } from "./fixtures";

const params = new URLSearchParams(window.location.search);
const scene = (params.get("scene") ?? "workspace") as Scene;
const theme = params.get("theme") === "light" ? "light" : "dark";

// src/main.tsx calls getCurrentWindow() at module top level.
mockWindows("main");
mockIPC((cmd, args) => handle(scene, theme, cmd, args as Record<string, unknown>), {
  shouldMockEvents: true,
});

// Theme cache read by bootstrapThemeFromCache() before settings load.
localStorage.setItem("meetphant.theme_preference", theme);

void import("/src/main.tsx");
