# Implementation Plan

## Part 1 — API Key auth: DONE (2026-10-09)
- Auth is handled by `providers/doubao/auth.rs`, with a unit test asserting the request carries only `X-Api-Key`. Credentials, IPC, the UI, the docs, and `api-shape.md` are updated.
- `cargo test --no-run` compiles. The harness fails with STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139), an environmental issue recorded in earlier tasks.
- Clippy fails only at the pre-existing `transcription_service.rs:207` and `:365`.
- `tsc` passes. vitest passes 50/50.
- Old-flow screenshots were removed from the guide; the image files remain on disk.

## Part 2 — Seed-ASR 2.0: DONE (2026-10-09)
- `ASYNC_RESOURCE_ID = volc.seedasr.auc`. Deleted `flash_client.rs` and `FLASH_MAX_AUDIO_BYTES`. `audio_format_from_path` moved to `async_client.rs`. Added `probe_auth` and `classify_probe`.
- `transcription_service.rs` now has a single TOS + async path, and TOS is required for every size. Tests migrated: added the small-file-via-TOS and small-file-without-TOS cases.
- `test_doubao` now calls `HttpAsyncClient::probe_auth`. Dropped the now-unused `base64` dependency from Cargo.toml.
- Added `#[allow(clippy::too_many_arguments)]` on `run_async_path`. The old `type_complexity` closure is gone. `cargo clippy --all-targets -D warnings` is clean.
- Updated UI copy (settings, import, recording, `formatError` TOS_NOT_CONFIGURED), the credentials guide, both READMEs, and the backend specs (api-shape, error-handling, quality, directory, database).
- `cargo test --no-run` OK. The harness still fails with 0xc0000139 (environmental). tsc OK; vitest 50/50.
- Pending manual checks: probe status codes with a valid and an invalid key, then pin them in `classify_probe`; real transcription of a small and a large file through TOS.

### Original plan
1. `async_client.rs`: set `ASYNC_RESOURCE_ID = "volc.seedasr.auc"`. Update the doc comment and the `resource_id_is_*` test. Move in `audio_format_from_path` along with its tests. Add `probe_auth` and a pure `classify_probe`, with tests.
2. Delete `providers/doubao/flash_client.rs` and update `providers/doubao/mod.rs` re-exports.
3. `transcription_service.rs`: switch to a single TOS + async path. Drop the flash parameters, helpers, and branches. Require TOS for every size. Update the stubs and tests: delete `flash_path_works_without_tos`, add a "small file without TOS → TOS_NOT_CONFIGURED" test, and make the small file go through TOS + async.
   - Note: the pre-existing clippy `too_many_arguments` at `:207` may disappear once the flash parameters are gone. Don't widen the refactor beyond that.
4. `meeting_service.rs`: delete `FLASH_MAX_AUDIO_BYTES` and fix the test at `:505`.
5. `settings_test_service.rs`: change `test_doubao` to `HttpAsyncClient::probe_auth`.
6. Frontend copy: `SettingsCredentials.tsx:482`, `TranscriptionImport.tsx:355`, `MeetingRecording.tsx:401`, `shared/lib/formatError.ts:4`.
7. Docs: credentials guide (enable 2.0, TOS required, self-check table, FAQ), README / README.zh-CN (feature bullet, credentials table, size table), `.trellis/spec/backend/api-shape.md`, `error-handling.md` (TOS_NOT_CONFIGURED row), `quality-guidelines.md` (dual-path wording).
8. Leftover scan: `grep -rnE "flash|Flash|极速|bigasr|auc_turbo|FLASH_MAX|20 MiB" src src-tauri/src docs README* .trellis/spec`.

## Validation
- `cargo test --no-run`; `cargo clippy --all-targets -- -D warnings` (report any remaining pre-existing failures).
- `node node_modules/typescript/bin/tsc --noEmit`; `node node_modules/vitest/vitest.mjs run`.
  - On this machine `npm`/`npx` fail with os error 193, so call node directly.
- Manual: valid key → 测试连接 OK; invalid key → auth error. Then pin the probe codes. Without TOS, a 1 MiB import → TOS prompt. With TOS, 1 MiB and >20 MiB imports → transcripts.

## Risky points
- Probe status-code classification is unverified (see design.md).
- Large deletion in `transcription_service.rs`. Keep the TOS-delete-after-success and timeout behavior intact; the existing async/TOS stub tests guard this.
- This shell (brush on Windows) has no perl/python, and `/workspace/...` paths in redirects can fail. Use the Edit/Write tools and relative paths.
