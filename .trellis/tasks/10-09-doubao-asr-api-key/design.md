# Design: 豆包 ASR 新版 API Key + 录音文件识别模型 2.0

## Part 1 — API Key auth (implemented)

- `providers/doubao/auth.rs`: `with_auth(req, creds)` adds `X-Api-Key`. It is the only place auth headers are set.
- `DoubaoCredentials { api_key }`; keyring `meetphant/doubao_api_key`. `migrate_legacy()` deletes `doubao_app_id` / `doubao_access_token` from the `meetphant` and `meetly` services.
- IPC: `SettingsUpdate.doubao_api_key`, `settings_test_doubao(doubao_api_key?)`, `doubao_configured` = key present.

## Part 2 — Seed-ASR 2.0, single standard path

### Pipeline (all sizes)
```
file (≤512 MiB) → require Doubao key + TOS config
  → TOS put → pre-signed GET URL
  → submit  POST /api/v3/auc/bigmodel/submit   X-Api-Resource-Id: volc.seedasr.auc
  → poll    POST /api/v3/auc/bigmodel/query    (same headers + X-Tt-Logid)
  → parse (asr_parse) → transcript → best-effort TOS delete
```
- `async_client.rs`: change `ASYNC_RESOURCE_ID` to `"volc.seedasr.auc"` and rename the doc comment to "Seed-ASR 2.0 standard". Leave the body unchanged (`model_name: bigmodel`, speaker info, `ssd_version: "200"`, hotwords → `corpus.context`).
- Delete `flash_client.rs`. Move `audio_format_from_path` into `async_client.rs` (or a small `audio.rs`) and keep its tests. Remove the flash re-exports from `mod.rs`.
- `transcription_service.rs`: drop `run_flash_path`, `read_audio_base64`, the `FlashRecognizer` params on `run_transcription_with_*`, and the `> FLASH_MAX_AUDIO_BYTES` branches. TOS becomes required at `start_transcription_job` for every size. `ASYNC_POLL_TIMEOUT` (45 min) is unchanged.
- `meeting_service.rs`: delete `FLASH_MAX_AUDIO_BYTES` and update its test (`:505`).

### Test connection probe (no TOS, no quota)
`HttpAsyncClient::probe_auth(creds)` sends a `query` request with a random UUID `X-Api-Request-Id` and the resource `volc.seedasr.auc`. Classification:

| Response | Result |
|---|---|
| Transport error | `ASR_PROVIDER_ERROR` "无法连接豆包" |
| HTTP 401/403, or status code in the auth/permission class (`45000010`-style "invalid key", "resource not granted") | `ASR_PROVIDER_ERROR` with `X-Api-Message`. Key invalid or 2.0 not enabled |
| Any other provider response (e.g. task-not-found / invalid-param status, 2xx) | OK: the key authenticated and the resource is reachable |

Put the decision in a pure function `classify_probe(http_status, status_code) -> ProbeVerdict` so it can be unit-tested. **Risk:** the exact status codes for "task not found" and "auth failed" are not in the fetched docs. Verify with one valid and one invalid key during the manual check, then pin the codes in the function and its tests. If they turn out to be indistinguishable, fall back to treating only HTTP 401/403 as an auth failure.

### Error contract changes
- `TOS_NOT_CONFIGURED` now covers any file when TOS is missing. Frontend copy changes from "大文件转写需要 TOS" to "转写需要火山 TOS". The code and branching are unchanged; `TranscriptionImport.tsx:321` and `MeetingRecording.tsx:318` already handle it.
- `ASR_PAYLOAD_TOO_LARGE` stays at 512 MiB.

## Compatibility
- Breaking for users without TOS: they could transcribe ≤20 MiB before, and now must configure TOS. This is called out in the docs' 升级必读 section and the settings hint.
- Users must enable 录音文件识别模型 2.0 (`volc.seedasr.auc`) in the new console. Enabling 1.0 or 极速版 is no longer needed.

## Rollback
Revert the commit. The 1.0 resource ids and the flash path are restored from VCS.
