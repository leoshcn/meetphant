# Logging Guidelines

> How the Rust backend (`src-tauri`) logs. Implementation: `src-tauri/src/logging.rs`.

---

## Setup

- **Library**: `tracing` + `tracing-subscriber` + `tracing-appender`.
- **Init**: `logging::init(app_log_dir)` runs at the **top** of `setup` in `lib.rs`, before migrations and DB open, so failures in those steps are captured.
- **Location**: `app.path().app_log_dir()`. On Windows this is `%LOCALAPPDATA%\com.meetphant.app\logs`.
  - Files are `meetphant.<date>.log`.
  - Rotation is daily, and the last **7** files are kept. There is no size-based rotation (tracing-appender limitation).
- **Writer**: non-blocking. The `WorkerGuard` lives in a `static OnceLock` for the whole process. Never drop it early, or buffered lines are lost.
- **Filter**: `warn,meetphant_lib=info` in release builds and `warn,meetphant_lib=debug` in debug builds. Override with the `MEETPHANT_LOG` env var, which uses EnvFilter syntax.
- **stderr**: always on in debug builds. In release builds it is used only as a fallback when the log directory is unusable.
- **Failure policy**: logging init must never block app startup.
- **User access**: Settings → About → 「打开日志文件夹」 calls `logs_open_dir`.

---

## Levels

| Level | Use for |
|-------|---------|
| `error` | Lost work or broken invariants: a job that cannot be marked failed, missing `AppState` in a worker, a poisoned DB mutex, a panicked blocking task |
| `warn` | Command failures (non-user-input codes), provider/TOS failures, best-effort cleanup failures, migration failures, FFmpeg download/encode failures, audio stream errors |
| `info` | Lifecycle milestones: logging init, migrations done, transcription stages (`upload.start/done`, `submit.done`, `poll.done`, `job.succeeded`) |
| `debug` | User-input command failures (`NOT_FOUND`, `SETTINGS_INVALID`, `INVALID_ARGUMENT`) and dev-only detail |

---

## Required Patterns

### Command failures

Every `#[tauri::command]` result ends with `.log_cmd("<command_name>")` (`CmdResultExt` in `error.rs`).

- It logs `cmd` and `code` only.
- It **never** logs `message`. Some messages carry upstream HTTP response text.

```rust
crate::db::with(&state.db, meeting_service::list_meetings).log_cmd("meetings_list")

run_blocking(move || { /* … */ }).await.log_cmd("summary_generate")
```

### Background jobs

Wrap the worker thread in a span with stable ids, and record stage timings as `elapsed_ms`:

```rust
let span = tracing::info_span!("transcription", job_id = %job_id, meeting_id = tracing::field::Empty);
let _entered = span.enter();
span.record("meeting_id", tracing::field::display(&ctx.meeting_id));
```

### No silent `let _ =` on fallible work that matters

- Persisting a terminal job state goes through `transcription_service::fail_job`. It logs the cause and also logs if the write itself fails.
- Best-effort cleanup (TOS delete, temp files) may ignore the error for control flow but must `warn!` with the error code.
- `let _ =` is still fine for truly irrelevant results, such as `reply.send` to a dropped receiver, test cleanup, or `OnceLock::set` races.

---

## Allowed fields

`cmd`, `code`, `mark_code`, `job_id`, `meeting_id`, `provider_task_id`, `object_key`, `file_size`, `elapsed_ms`, `outcome`, `version`, `kind` (an `io::ErrorKind`), and cpal/OS device error text.

## Forbidden — never log

- API keys and tokens: Doubao `X-Api-Key`, DashScope key, TOS AK/SK
- Pre-signed URLs (`audio_url`) or any `X-Tos-Signature` query
- Request or response bodies, transcript text, summary text, `context_text`, hotwords
- `AppErrorDto.message`
- Audio content
- Absolute filesystem paths. They contain the OS username, so log `io::ErrorKind` instead.

---

## Review checklist

- [ ] New command ends with `.log_cmd("…")`
- [ ] New background thread has a span and logs terminal outcome
- [ ] No new `let _ =` on persistence or provider calls without a log
- [ ] Fields only from the allowed list; grep the diff for `audio_url`, `api_key`, `secret`, `message`
