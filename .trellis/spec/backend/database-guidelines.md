# Database Guidelines



## Settings table



Migrations:



- `src-tauri/src/db/migrations/001_settings.sql` — base singleton

- Idempotent TOS columns via `ensure_tos_settings_columns` in `db/pool.rs` (documented by `004_tos_settings.sql`)

- Idempotent `recording_dir` via `ensure_recording_dir_column` (`006_recording_dir.sql`)

- Idempotent `theme_preference` via `ensure_theme_preference_column` (`007_theme_preference.sql`)

- Idempotent summary LLM columns via `ensure_summary_llm_columns` (`008_summary_llm.sql`)



Singleton row `id = 1`:



| Column | Contents |

|--------|----------|

| `hotwords` | TEXT NOT NULL DEFAULT `'[]'` (JSON string array) |

| `context_text` | TEXT NOT NULL DEFAULT `''` |

| `tos_region` | TEXT NOT NULL DEFAULT `''` (non-secret) |

| `tos_bucket` | TEXT NOT NULL DEFAULT `''` (non-secret) |

| `tos_endpoint` | TEXT NOT NULL DEFAULT `''` (optional; empty → SDK default) |

| `recording_dir` | TEXT NOT NULL DEFAULT `''` (empty → Documents/Meetphant/Recordings) |

| `theme_preference` | TEXT NOT NULL DEFAULT `'system'` (`system` \| `light` \| `dark`) |

| `summary_llm_provider` | TEXT NOT NULL DEFAULT `'dashscope'` (preset id) |

| `summary_llm_base_url` | TEXT NOT NULL DEFAULT `''` (empty → preset default base URL) |

| `summary_llm_model` | TEXT NOT NULL DEFAULT `''` (empty → preset recommended model) |



**Never** store Doubao, summary LLM, or TOS secrets in SQLite — those live in the OS keyring (`meetphant` service; summary key account `summary_llm_api_key`).



Access via `src-tauri/src/services/settings_service.rs` and `db/pool.rs`.



## Jobs



`jobs.provider_task_id` stores the Doubao Seed-ASR 2.0 submit `X-Api-Request-Id` (null until submit succeeds).



## Transcripts



Migrations:



- `002_meetings_jobs.sql` — base `transcripts(meeting_id, text, raw_json)`

- Idempotent speaker columns via `ensure_transcript_speaker_columns` in `db/pool.rs` (documented by `005_transcript_speakers.sql`)



| Column | Contents |

|--------|----------|

| `text` | Rendered full transcript (includes speaker display names when diarized) |

| `raw_json` | Raw ASR response |

| `segments_json` | JSON array of `{ speaker_id, text }` (nullable / empty when no diarization) |

| `speaker_names_json` | JSON object map `speaker_id` → display name |

