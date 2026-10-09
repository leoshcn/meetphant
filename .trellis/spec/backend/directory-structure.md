# Directory Structure

> How Rust / Tauri backend code is organized in Meetphant.

---

## Overview

Backend logic lives in `src-tauri/src/`. Commands are thin; services own business rules; `db` owns SQLite; `providers` own external HTTP / SDK clients.

---

## Directory Layout

```
src-tauri/src/
├── main.rs
├── lib.rs
├── error.rs
├── commands/
│   ├── health.rs
│   ├── settings.rs
│   ├── meetings.rs
│   ├── jobs.rs
│   ├── summary.rs
│   └── recording.rs
├── services/
│   ├── settings_service.rs
│   ├── meeting_service.rs
│   ├── recording_service.rs
│   ├── transcription_service.rs
│   ├── summary_service.rs
│   └── credentials.rs
├── providers/
│   ├── doubao/
│   │   ├── mod.rs
│   │   ├── auth.rs
│   │   ├── async_client.rs
│   │   └── hotwords.rs
│   ├── tos/
│   │   └── mod.rs
│   └── openai_compat/
│       ├── mod.rs
│       ├── client.rs      # LlmConfig, prompts, HttpChatClient (generate + test_connection)
│       └── presets.rs     # provider presets (single source of truth)
├── db/
│   ├── pool.rs
│   └── migrations/
│       ├── 001_settings.sql
│       ├── 002_meetings_jobs.sql
│       ├── 003_summaries.sql
│       ├── 004_tos_settings.sql
│       ├── …
│       └── 008_summary_llm.sql
└── models/
    ├── settings.rs
    ├── meeting.rs
    ├── job.rs
    └── summary.rs
```

---

## Module Organization

- **commands/** — IPC edge only.
- **services/** — validation + persistence + job orchestration (incl. TOS upload + Seed-ASR 2.0).
- **providers/** — Doubao Seed-ASR 2.0 (submit/query + X-Api-Key auth), TOS object storage, OpenAI-compatible summary LLM (DashScope / DeepSeek / OpenAI / Moonshot / Zhipu / Ark / custom); no SQLite.
- **db/** — migrations + connection.
- **models/** — serde DTOs shared across IPC.

---

## Anti-Patterns

- Doubao / TOS HTTP or SDK calls inside command handlers (keep under `providers/`).
- Returning unstructured `String` errors from commands.
- Storing Doubao, summary LLM, or TOS secrets in SQLite.
