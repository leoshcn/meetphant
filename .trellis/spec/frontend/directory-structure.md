# Directory Structure

> How frontend code is organized in Meetphant (Tauri + React + TypeScript).

---

## Overview

Meetphant UI lives under `src/` (Vite + React). Feature folders own screens and feature-local components; shared UI and the typed IPC client stay outside features.

---

## Directory Layout

```
src/
├── app/                    # AppShell
├── pages/
│   ├── home/
│   └── settings/
├── features/
│   ├── settings-hotwords/
│   ├── settings-credentials/
│   ├── settings-recording/
│   ├── settings-ffmpeg/
│   ├── settings-appearance/
│   ├── settings-about/
│   ├── app-update/
│   ├── meeting-recording/
│   ├── recorder-widget/
│   ├── transcription-import/
│   ├── meeting-summary/
│   └── meeting-sidebar/
├── shared/
│   ├── ui/
│   ├── hooks/
│   └── lib/
├── ipc/
│   ├── client.ts
│   ├── types.ts
│   └── commands/
├── styles/
└── main.tsx
```

Evidence: `src/app/AppShell.tsx`, `src/pages/settings/SettingsPage.tsx`, `src/features/settings-hotwords/SettingsHotwords.tsx`, `src/features/app-update/`, `src/features/meeting-summary/`, `src/features/meeting-sidebar/`, `src/features/recorder-widget/`, `src/ipc/client.ts`.

---

## Module Organization

- **pages/** — composition only.
- **features/** — capability UI; do not import other features’ internals.
- **ipc/** — the only place that calls Tauri `invoke`.

---

## Naming Conventions

| Kind | Rule | Example |
|------|------|---------|
| Components | PascalCase | `HotwordList.tsx` |
| Hooks | `use` + camelCase | (add as needed) |
| IPC modules | domain noun | `ipc/commands/settings.ts` |

---

## Anti-Patterns

- Calling `invoke` outside `src/ipc/`.
- Flat `components/` dumping ground for screens.
