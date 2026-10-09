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
cd src-tauri && cargo fmt --check
cd src-tauri && cargo clippy --all-targets -- -D warnings
cd src-tauri && cargo test
```

CI (`.github/workflows/ci.yml`, windows-latest) runs these on every push to `main` and every PR, after `npm run typecheck`, `npm test`, and `npm run build`. The frontend build is required because `dist` is gitignored and tauri-codegen panics at compile time when `frontendDist` is missing.

### Windows test harness: Common-Controls manifest

Tauri's default build embeds the Common-Controls v6 manifest into the app exe only. The `cargo test` harness then loads comctl32 v5 and aborts with `STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139)` before any test runs. `src-tauri/build.rs` therefore uses `WindowsAttributes::new_without_app_manifest()` and declares the same dependency for every MSVC artifact via `/MANIFEST:EMBED` + `/MANIFESTDEPENDENCY` link args.

- Do not revert to plain `tauri_build::build()`. Doing so silently disables every Rust test.
- 0xc0000139 is not an environmental issue. Check this first.

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
