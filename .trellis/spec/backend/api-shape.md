# API Shape

> App-internal API is Tauri IPC (commands), not HTTP.

---

## Scope / Trigger

Applies when adding or changing any `#[tauri::command]`, frontend `invoke` wrapper, or shared DTO between `src/` and `src-tauri/`.

---

## Signatures

### Implemented commands

| Command | Request | Response | Source |
|---------|---------|----------|--------|
| `app_health` | (none) | `{ status, version }` | `src-tauri/src/commands/health.rs`, `src/ipc/commands/health.ts` |
| `logs_open_dir` | (none) | `void` — opens `app_log_dir()` in the system file manager; `IO_ERROR` on failure | same |
| `settings_get` | (none) | `Settings` | `src-tauri/src/commands/settings.rs` |
| `settings_update` | `SettingsUpdate` | `Settings` | same |
| `settings_clear_doubao_credentials` | (none) | `Settings` | same |
| `settings_clear_summary_llm_credentials` | (none) | `Settings` | same — clears only the summary LLM API key (keyring); provider / base URL / model are kept |
| `settings_clear_tos_credentials` | (none) | `Settings` | same |
| `settings_test_doubao` | optional `{ doubao_api_key? }` | `{ ok: true }` | same — merges overrides with keyring; **does not persist** |
| `settings_test_tos` | optional `{ tos_access_key_id?, tos_secret_access_key?, tos_region?, tos_bucket?, tos_endpoint? }` | `{ ok: true }` | same — merges with keyring/SQLite; HeadBucket probe; **does not persist** |
| `settings_test_summary_llm` | optional `{ api_key?, provider?, base_url?, model? }` | `{ ok: true }` | same — merges with keyring/SQLite; POST `{base_url}/chat/completions` with the configured model, one `ping` user message, output cap 1 token (`max_tokens`; `max_completion_tokens` for `openai`); if merged provider/base URL differ from saved, `api_key` is required (saved key never sent to a new endpoint); **does not persist** |
| `meetings_create` | (none) | `Meeting` (draft: `title` =「未命名项目」, `source_path` = `""`) | `src-tauri/src/commands/meetings.rs` |
| `meetings_create_from_file` | `{ path: string }` | `Meeting` | `src-tauri/src/commands/meetings.rs` |
| `meetings_attach_source` | `{ meeting_id: string, path: string }` | `Meeting` — only when draft (`source_path` empty); keeps custom title, else file stem | same |
| `meetings_list` | (none) | `Meeting[]` (created_at DESC) | same |
| `meetings_get` | `{ meeting_id: string }` | `Meeting` | same |
| `meetings_rename` | `{ meeting_id: string, title: string }` | `Meeting` | same |
| `meetings_delete` | `{ meeting_id: string }` | `void` | same |
| `meetings_get_transcript` | `{ meeting_id: string }` | `Transcript` | same |
| `meetings_update_speakers` | `{ meeting_id: string, speaker_names: Record<string, string> }` | `Transcript` | same |
| `jobs_start_transcription` | `{ meeting_id: string }` | `Job` | `src-tauri/src/commands/jobs.rs` |
| `jobs_get` | `{ job_id: string }` | `Job` | same |
| `summary_generate` | `{ meeting_id: string, language: "zh-CN" \| "en" \| "zh-en" }` | `Summary` | `src-tauri/src/commands/summary.rs` |
| `summary_get` | `{ meeting_id: string }` | `Summary` | same |
| `record_list_input_devices` | (none) | `{ devices: [{ id, name, is_default }], default_id }` | `src-tauri/src/commands/recording.rs` |
| `record_start` | `{ device_id?: string \| null }` | `{ path, device_name, output_device_name }` — starts mic + WASAPI loopback mix | same |
| `record_stop` | (none) | `{ path, duration_ms }` | same |
| `record_status` | (none) | `{ state, path, started_at, device_name, output_device_name, mic_level, system_level }` — levels are smoothed \[0,1\] capture meters | same |
| `recording_hide_to_tray` | (none) | `void` — show recording tray, hide `main` | `src-tauri/src/commands/tray.rs` |
| `recording_restore_from_tray` | (none) | `void` — show/focus `main`, hide recording tray | same |
| `recording_hide_tray` | (none) | `void` — hide tray only | same |

### Command execution model

Tauri 2 runs sync `#[tauri::command]` fns on the **main thread**.

- Any command that does network IO, runs a subprocess (FFmpeg), touches audio devices, or reads audio files must be `async fn` and wrap its work in `commands::run_blocking` (`spawn_blocking`).
  - Pass `AppHandle` and call `app.state::<AppState>()` inside the closure.
  - Do **not** use `#[tauri::command(async)]` on a sync fn. It runs the body on a tokio worker, where `reqwest::blocking` is unsafe.
- Pure-SQLite commands stay sync and use `crate::db::with(&state.db, f)`.
- Never hold the DB lock (`db::lock` / `db::with`) across a network call. Read inputs, release the lock, call the provider, then re-lock to write. For example, `summary_service::generate_summary` takes `&Mutex<Connection>`.
- Every command's result ends with `.log_cmd("<command_name>")` (see logging-guidelines.md).

### Window events (no command / DTO change)

| Event | Direction | Payload | Source |
|-------|-----------|---------|--------|
| `recording:close-requested` | Rust → `main` | none | `src-tauri/src/lib.rs` `on_window_event` when main close is blocked while recording |
| `recording:focus-request` | tray / `recorder-widget` → `main` | none | tray left-click/menu「打开 Meetphant」or widget「打开 Meetphant」; AppShell sets `screen=home` |

### Envelope

Rust: `CmdResult<T> = Result<T, AppErrorDto>` in `src-tauri/src/error.rs`.  
TS: `AppError` + `normalizeError` in `src/ipc/client.ts`. Success is bare `T` (no `{ ok: true }` wrap).

```ts
// src/ipc/types.ts
type Settings = {
  hotwords: string[];
  context_text: string;
  doubao_configured: boolean;
  /** True when a summary LLM API key is in the keyring (key never returned). */
  summary_llm_configured: boolean;
  /** Preset id: dashscope | deepseek | openai | moonshot | zhipu | ark | custom. */
  summary_llm_provider: string;
  /** Effective base URL (stored value, else preset default). */
  summary_llm_base_url: string;
  /** Effective model (stored value, else preset recommendation; may be "" only if unconfigured). */
  summary_llm_model: string;
  /** True when TOS AK+SK (keyring) and region+bucket (SQLite) are all present. */
  tos_configured: boolean;
  /** Non-secret; echoed by settings_get. */
  tos_region: string;
  tos_bucket: string;
  /** Optional custom endpoint; empty → default `https://tos-{region}.volces.com`. */
  tos_endpoint: string;
  /** User override; empty → default Documents/Meetphant/Recordings. */
  recording_dir: string;
  /** Effective path after resolving empty default. */
  recording_dir_resolved: string;
  /** UI theme preference: `system` | `light` | `dark`. */
  theme_preference: string;
};
type SettingsUpdate = {
  hotwords?: string[];
  context_text?: string;
  /** Write-only Doubao new-console API Key (sent as `X-Api-Key`); never returned by settings_get. */
  doubao_api_key?: string;
  /** Write-only summary LLM API key; never returned by settings_get. */
  summary_llm_api_key?: string;
  /** Known preset id. Changing it without base_url/model resets them to the preset defaults. */
  summary_llm_provider?: string;
  /** Must be an http(s):// URL with a host; "" is rejected. */
  summary_llm_base_url?: string;
  /** Must be non-empty. */
  summary_llm_model?: string;
  /** Write-only TOS Access Key Id; never returned by settings_get. */
  tos_access_key_id?: string;
  /** Write-only TOS Secret Access Key; never returned by settings_get. */
  tos_secret_access_key?: string;
  tos_region?: string;
  tos_bucket?: string;
  tos_endpoint?: string;
  /** Absolute path or empty to reset default. */
  recording_dir?: string;
  /** UI theme preference: `system` | `light` | `dark`. */
  theme_preference?: string;
};
/** Optional write-only overrides for settings_test_*; empty/omit → use saved. Never persisted by test commands. */
type SettingsTestDoubaoOverrides = {
  doubao_api_key?: string;
};
type SettingsTestTosOverrides = {
  tos_access_key_id?: string;
  tos_secret_access_key?: string;
  tos_region?: string;
  tos_bucket?: string;
  tos_endpoint?: string;
};
type SettingsTestSummaryLlmOverrides = {
  api_key?: string;
  provider?: string;
  base_url?: string;
  model?: string;
};
type SettingsTestResult = { ok: true };
type Meeting = {
  id: string;
  source_path: string;
  title: string | null;
  created_at: string;
};
type Transcript = {
  meeting_id: string;
  text: string;
  segments: { speaker_id: string; text: string }[];
  speaker_names: Record<string, string>;
};
type Job = {
  id: string;
  meeting_id: string;
  kind: "transcription" | string;
  status: "running" | "succeeded" | "failed" | string;
  error_code: string | null;
  error_message: string | null;
  created_at: string;
  updated_at: string;
};
type Summary = {
  meeting_id: string;
  key_points: string[];
  action_items: string[];
  decisions: string[];
  language: "zh-CN" | "en" | "zh-en";
  created_at: string;
};
```

`meetings_rename` rejects empty/whitespace titles (`INVALID_ARGUMENT`).  
`meetings_delete` removes `summaries` / `transcripts` / `jobs` / `meetings` rows; does **not** delete the local audio file.  
`meetings_update_speakers` re-renders `text`, persists `speaker_names`, deletes that meeting’s summary; fails with `TRANSCRIPT_NO_SPEAKERS` when segments are empty.  
`summary_generate` requires supported `language`; unsupported → `INVALID_ARGUMENT`.
---

## Contracts

| Field | Consumer | Must NOT |
|-------|----------|----------|
| `hotwords` | Doubao Seed-ASR 2.0 submit (`request.corpus.context`) | Be required for summary |
| `context_text` | Summary LLM prompt | Be sent to Doubao ASR |
| `doubao_api_key` | OS keyring via `settings_update`; sent only as `X-Api-Key` | Appear in any `settings_get` / logs |
| `doubao_configured` | UI status only | Imply returning secret material |
| `summary_llm_api_key` | OS keyring via `settings_update`; sent only as `Authorization: Bearer` to the configured base URL | Appear in any `settings_get` / logs / error messages; follow a provider or base URL change without being re-entered |
| `summary_llm_configured` | UI status only | Imply returning secret material |
| `summary_llm_provider` / `summary_llm_base_url` / `summary_llm_model` | SQLite + summary client | Store secrets |
| `tos_access_key_id` / `tos_secret_access_key` | OS keyring via `settings_update` | Appear in any `settings_get` / logs |
| `tos_configured` | UI status only | Imply returning AK/SK |
| `tos_region` / `tos_bucket` / `tos_endpoint` | SQLite + TOS client | Store secrets |

### Credentials

| Store | Rule |
|-------|------|
| OS keyring (`meetphant` / `doubao_api_key`) | Write via settings; read only inside provider. Old-console `doubao_app_id` / `doubao_access_token` are unsupported and deleted on startup |
| OS keyring (`meetphant` / `summary_llm_api_key`) | Write via settings; read by summary service / test merge only. On startup, if empty, copied from `dashscope_api_key` (`meetphant`, then legacy `meetly`); the old entry is **kept** for rollback. `settings_clear_summary_llm_credentials` also deletes the old `dashscope_api_key` entries so a cleared key is not re-migrated |
| OS keyring (`meetphant` / `tos_access_key_id`, `tos_secret_access_key`) | Write via settings; read only inside TOS provider. Migrated once from the legacy `meetly` service |
| SQLite `settings` | Never stores Doubao, summary LLM, or TOS secrets; may store `tos_region` / `tos_bucket` / `tos_endpoint` and `summary_llm_provider` / `summary_llm_base_url` / `summary_llm_model` |

### Summary LLM providers

All presets use the OpenAI Chat Completions protocol (`POST {base_url}/chat/completions`, trailing `/` tolerated). Source of truth: `src-tauri/src/providers/openai_compat/presets.rs`; UI mirror: `src/features/settings-credentials/summaryLlm.ts`.

| id | Default base URL | Default model | `response_format: json_object` | Extra body |
|----|------------------|---------------|-------------------------------|------------|
| `dashscope` (default) | `https://dashscope.aliyuncs.com/compatible-mode/v1` | `qwen3.7-plus` | yes | `enable_thinking: false` |
| `deepseek` | `https://api.deepseek.com/v1` | `deepseek-chat` | yes | — |
| `openai` | `https://api.openai.com/v1` | (user fills) | yes | — |
| `moonshot` | `https://api.moonshot.cn/v1` | (user fills) | yes | — |
| `zhipu` | `https://open.bigmodel.cn/api/paas/v4` | (user fills) | yes | — |
| `ark` | `https://ark.cn-beijing.volces.com/api/v3` | (user fills) | no | — |
| `custom` | (user fills) | (user fills) | no | — |

Empty stored `summary_llm_base_url` / `summary_llm_model` resolve to the preset defaults, so upgraded installs keep DashScope + `qwen3.7-plus`. Unknown stored provider ids resolve to `custom`. Responses wrapped in a ```json fence are accepted.

`tos_configured === true` only when AK, SK, region, and bucket are all present (endpoint optional).  
`settings_clear_tos_credentials` clears keyring secrets and wipes region/bucket/endpoint.

### DB

- `settings` singleton (`id = 1`): `hotwords`, `context_text`, plus TOS non-secrets — `001_settings.sql` + idempotent `004` / `ensure_tos_settings_columns`; summary LLM non-secrets — idempotent `008` / `ensure_summary_llm_columns`
- `meetings`, `jobs`, `transcripts` — `002_meetings_jobs.sql` (`jobs.provider_task_id` used for Doubao async request id)
- `transcripts.segments_json` / `speaker_names_json` — idempotent via `ensure_transcript_speaker_columns` (`005_transcript_speakers.sql`)
- `summaries` — `003_summaries.sql`

### Size cap & path (single path)

| Constant | Value | Role |
|----------|-------|------|
| `ASYNC_MAX_AUDIO_BYTES` | 512 MiB | Hard reject for import / start / execute |

Every size uses the same path at `jobs_start_transcription` / execute (Seed-ASR 2.0 only accepts `audio.url`):

| Size | Path |
|------|------|
| ≤ 512 MiB | TOS upload → pre-signed GET → Doubao Seed-ASR 2.0 submit/query (`X-Api-Resource-Id: volc.seedasr.auc`, auth `X-Api-Key`) |
| > 512 MiB | `ASR_PAYLOAD_TOO_LARGE` |

`meetings_create_from_file` rejects only **> 512 MiB** so large files can be stored; TOS is required (for every size) at transcription start.
`meetings_create` inserts a draft (`source_path` empty, title「未命名项目」). `meetings_attach_source` binds a file to a draft only; rejects if `source_path` already set (`INVALID_ARGUMENT`).

Async poll window: **45 minutes** client-side (`ASR_TIMEOUT` on exceed). Pre-signed URL TTL ≥ poll window (2 h).

---

## Validation & Error Matrix

| Condition | Code | Behavior |
|-----------|------|----------|
| Empty / whitespace hotword | `SETTINGS_INVALID` | No DB write |
| SQLite failure | `DB_ERROR` | Generic message (no filesystem paths) |
| Missing Doubao credentials | `ASR_NOT_CONFIGURED` | No provider call |
| Incomplete TOS merge for test | `SETTINGS_INVALID` | Inline on credentials test |
| `settings_test_doubao` auth probe rejected | `ASR_PROVIDER_ERROR` | Query with random request id; HTTP 401/403 → failure; needs no TOS / quota |
| Audio file > 512 MiB | `ASR_PAYLOAD_TOO_LARGE` | Reject before create / start |
| Attach source to non-draft meeting | `INVALID_ARGUMENT` | `meetings_attach_source` no-op |
| Any size without complete TOS config | `TOS_NOT_CONFIGURED` | Fail fast at start; no job created |
| TOS put / pre-sign failure | `TOS_UPLOAD_ERROR` | Job → `failed` (if already started) |
| Cannot read audio file | `IO_ERROR` | Safe message |
| Provider non-success (submit / query) | `ASR_PROVIDER_ERROR` | Job → `failed` |
| Async poll exceeds 45 min | `ASR_TIMEOUT` | Job → `failed` |
| Transcript missing / not ready for summary | `SUMMARY_NOT_READY` | No LLM call |
| Missing summary LLM API key (or empty effective base URL / model) | `SUMMARY_NOT_CONFIGURED` | No LLM call |
| Unknown `summary_llm_provider`; `summary_llm_base_url` empty / not `http(s)://` with host; `summary_llm_model` empty | `SETTINGS_INVALID` (`details.field`) | No DB or keyring write |
| Provider / base URL change while a key is saved, without a new key (`settings_update` / `settings_test_summary_llm`) | `SETTINGS_INVALID` (`details.field = summary_llm_api_key`) | No write / no request |
| Summary LLM HTTP / network / JSON failure | `SUMMARY_PROVIDER_ERROR` | Message names the provider label; 401/403 →「API Key 无效或无权限」; other statuses include `HTTP <code>` (+ short upstream `error.message`, key redacted; never for 401/403); `details.http_status`; no partial persist |
| Unknown meeting/job/summary id | `NOT_FOUND` | — |
| Lock poisoned / unknown | `INTERNAL` | Safe message |

---

## Good / Base / Bad Cases

| Case | Expect |
|------|--------|
| Good | Import with TOS → TOS upload → Seed-ASR 2.0 job → transcript → summary |
| Base | Empty DB → settings defaults + all `*_configured: false` + empty TOS non-secret fields + `summary_llm_provider = dashscope` with DashScope base URL / `qwen3.7-plus` |
| Bad | `hotwords: [""]` → `SETTINGS_INVALID`; no Doubao → `ASR_NOT_CONFIGURED`; any file without TOS → `TOS_NOT_CONFIGURED`; no summary key → `SUMMARY_NOT_CONFIGURED`; `summary_llm_base_url: "ftp://x"` → `SETTINGS_INVALID` |

---

## Tests Required

- Rust: settings (incl. Doubao / summary LLM / TOS configured flags, no secret echo), summary LLM validation + endpoint-change key rule + preset defaults on upgrade, keyring copy-migration, hotwords builder, auth header (`X-Api-Key` only), Seed-ASR resource id, probe classification, async stub job transitions (small + large file via TOS, no-TOS rejection), TOS stub put/presign, async poll timeout → `ASR_TIMEOUT`, summary prompt/parse with stub generator, chat URL join, per-preset body (`response_format` / `enable_thinking`), local HTTP server tests for generate / test connection status mapping.
- TS: ipc wrappers for settings (incl. `settings_clear_tos_credentials`, `settings_clear_summary_llm_credentials`, `settings_test_*`) / meetings / jobs / summary; summary LLM preset mirror + endpoint-change helper.

---

## Wrong vs Correct

Wrong: UI `invoke` outside `src/ipc`; `context_text` on ASR submit; secrets in SQLite or `settings_get`; rusqlite Display leaked to UI; sending `audio.data` base64 or the 1.0 / flash resource ids; skipping TOS for small files.

Correct: wrappers in `src/ipc/commands/*`; hotwords→ASR / context→summary; keyring secrets; dual-path size gates; sanitized errors.
