# Quality Guidelines

> Backend quality bar and test strategy (`src-tauri`).

---

## Test Strategy

| Layer | Tool | What |
|-------|------|------|
| Unit / service | `cargo test` | validation, persist, error mapping |
| Provider (later) | `cargo test` + stub HTTP | hotwords in ASR body; exclude `context_text` |
| Manual | README checklist | `tauri dev` window |

```bash
cd src-tauri && cargo test
cd src-tauri && cargo clippy -- -D warnings
```

Evidence: settings + error + transcription single path (TOS + Seed-ASR 2.0 / no-TOS rejection / timeout) + TOS stub + summary stubs under `cargo test`.

---

## Forbidden Patterns

- `unwrap()` on fallible IO in command handlers.
- Unstructured `String` command errors.
- Forwarding rusqlite Display to IPC (path leak).
- Logging TOS AK/SK or pre-signed URL signatures.

---

## Required Patterns

- `CmdResult<T>` on all commands.
- Migrations under `src-tauri/src/db/migrations/`.
- Size cap: `ASYNC_MAX_AUDIO_BYTES` (512 MiB). Every transcription requires TOS (Seed-ASR 2.0 is URL-only).

---

## Code Review Checklist

- [ ] Stable error codes (incl. `TOS_*` / `ASR_TIMEOUT`)
- [ ] Hotwords vs context consumers correct (Seed-ASR 2.0 submit)
- [ ] Tests for invalid settings + DB mapping + TOS-required path
- [ ] No credentials in source / `settings_get`
