# Error Handling

> How errors cross the Tauri IPC boundary and surface in the Meetly UI.

---

## Scope / Trigger

All Rust command handlers, SQLite access, provider calls, and frontend `invoke` wrappers.

---

## Signatures

```rust
// src-tauri/src/error.rs
pub struct AppErrorDto {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}
pub type CmdResult<T> = Result<T, AppErrorDto>;
```

```ts
// src/ipc/client.ts
export type AppError = { code: string; message: string; details?: unknown };
export function normalizeError(err: unknown): AppError;
```

Helpers: `settings_invalid`, `invalid_argument`, `db_error`, `internal`, `not_found`, `asr_not_configured`, `asr_payload_too_large`, `asr_provider_error`, `asr_timeout`, `tos_not_configured`, `tos_upload_error`, `io_error`, `summary_not_ready`, `summary_not_configured`, `summary_provider_error`, `transcript_no_speakers`, `record_no_device`, `record_busy`, `record_not_active`, `record_device_error`.

---

## Contracts

1. Commands return `CmdResult<T>`.
2. `message` is user-safe; never tokens, base64 audio, pre-signed URL query strings with signatures at info logs, or absolute secret paths.
3. `From<rusqlite::Error>` / `From<serde_json::Error>` → `DB_ERROR` with **fixed** messages (do not forward Display).
4. Frontend normalizes unknown rejects to `{ code: "INTERNAL", message: "Unexpected error" }`.
5. Transcription jobs persist terminal failures as `status=failed` with non-empty `error_code` / `error_message` for UI polling (covers TOS upload failures, Seed-ASR submit/query failures, and `ASR_TIMEOUT`).
6. Summary generation failures return typed codes; do not persist partial summary on parse/API failure.
7. Best-effort TOS object delete after successful transcript must **not** flip the job to failed.

---

## Validation & Error Matrix

| Fault | Code | UI |
|-------|------|-----|
| Validation | `SETTINGS_INVALID` / `INVALID_ARGUMENT` | Inline / form |
| SQLite | `DB_ERROR` | Toast |
| Missing ASR credentials | `ASR_NOT_CONFIGURED` | Prompt to settings |
| File too large (> 512 MiB) | `ASR_PAYLOAD_TOO_LARGE` | Inline / job error |
| Any file without TOS | `TOS_NOT_CONFIGURED` | Prompt to configure TOS |
| TOS put / pre-sign failure | `TOS_UPLOAD_ERROR` | Job error |
| File read failure | `IO_ERROR` | Inline / job error |
| Doubao API failure (submit / query / test probe) | `ASR_PROVIDER_ERROR` | Job error |
| Async poll > 45 minutes | `ASR_TIMEOUT` | Job error |
| Transcript not ready for summary | `SUMMARY_NOT_READY` | Inline / prompt to finish ASR |
| Missing DashScope key | `SUMMARY_NOT_CONFIGURED` | Prompt to settings |
| Qwen API / invalid JSON | `SUMMARY_PROVIDER_ERROR` | Inline |
| No speaker segments to rename | `TRANSCRIPT_NO_SPEAKERS` | Inline |
| No audio input device | `RECORD_NO_DEVICE` | Inline |
| Recording already active | `RECORD_BUSY` | Inline |
| Stop with no active recording | `RECORD_NOT_ACTIVE` | Inline |
| Device / stream failure | `RECORD_DEVICE_ERROR` | Inline / settings |
| Unknown id | `NOT_FOUND` | Inline |
| Unexpected | `INTERNAL` | Generic toast |

---

## Good / Base / Bad Cases

| Case | Behavior |
|------|----------|
| Good | `SETTINGS_INVALID` / `ASR_*` / `TOS_*` / `SUMMARY_*` codes preserved through `normalizeError` |
| Base | `app_health` → `Ok` |
| Bad | String-only rejects without `code` — client maps to `INTERNAL` |

---

## Tests Required

- `error.rs`: rusqlite mapping has no path separators in message.
- `client.test.ts`: preserves `{ code, message }`; string → `INTERNAL`.
- Job / provider tests assert stable ASR / TOS codes without leaking payloads.
- Async poll timeout persists `ASR_TIMEOUT` on the job row.
- Summary tests assert `SUMMARY_*` codes for not ready / not configured / parse failure.

---

## Wrong vs Correct

Wrong: `Err(e.to_string())` from commands; logging access tokens, base64 audio, or TOS secrets; mapping missing TOS to `IO_ERROR`.

Correct: typed `AppErrorDto`; UI branches on `code`; `TOS_NOT_CONFIGURED` for incomplete large-file config.
