# Quality Guidelines

> Frontend quality bar and test strategy (`src/`).

---

## Test Strategy

| Layer | Tool | What |
|-------|------|------|
| Unit | Vitest | `ipc/client` normalize; command wrappers |
| E2E | Deferred | No Tauri driver required for scaffold |
| Manual | checklist | Settings save; copy for hotwords/context |

```bash
npm test
npm run typecheck
```

Evidence: `src/ipc/client.test.ts`, `src/ipc/commands/*.test.ts` (7 tests).

---

## Forbidden Patterns

- Raw `invoke` outside `src/ipc/`.
- Storing Doubao tokens in frontend state.
- Putting `context_text` on transcription invoke payloads.

---

## Required Patterns

- Typed wrappers in `src/ipc/commands/*`.
- Settings copy: 热词→转写, 上下文→摘要 (`SettingsHotwords.tsx` / settings page).

---

## Code Review Checklist

- [ ] IPC only via `src/ipc`
- [ ] Vitest covers new wrappers
- [ ] User-visible errors use `AppError.message`

---

## Rendering the UI Without Tauri (screenshots / demos)

`npm run screenshots` (`scripts/screenshots/`) renders the real React app in a browser with mocked IPC and writes `website/assets/screenshots/`.

- Install `mockWindows("main")` **before** importing `/src/main.tsx` — it calls `getCurrentWindow()` at module top level.
- Use `mockIPC(handler, { shouldMockEvents: true })`; components call `listen(...)`, which otherwise fails as an unhandled `plugin:event|listen`.
- Fixtures must follow `src/ipc/types.ts`; when an IPC command or its shape changes, update `scripts/screenshots/fixtures.ts` too (the script exits non-zero on unmocked commands / visible error alerts).
- Demo entry lives outside `src/` so `vite build` and `tsc` never include it.
- Mobile crops (`*-mobile.png`) are clipped by element locators; the website `<picture>` width/height attrs hardcode their sizes — update both if a crop changes size.
- Re-run `npm run screenshots` after visible UI changes; screenshots are committed, not generated in CI (CJK font rendering differs).
