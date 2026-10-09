use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use chrono::Utc;
use rusqlite::Connection;
use uuid::Uuid;

use crate::error::{AppErrorDto, CmdResult};
use crate::models::{render_transcript_text, Meeting, Transcript, TranscriptSegment};
use crate::providers::doubao::parse_asr_transcript;

/// Hard reject cap for import / Seed-ASR 2.0 path (512 MiB).
pub const ASYNC_MAX_AUDIO_BYTES: u64 = 512 * 1024 * 1024;

/// Default title for draft meetings created via「新建项目」.
pub const DEFAULT_DRAFT_TITLE: &str = "未命名项目";

fn validate_audio_path(path: &str) -> CmdResult<&Path> {
    let path = path.trim();
    if path.is_empty() {
        return Err(AppErrorDto::io_error("Audio path is empty"));
    }

    let file_path = Path::new(path);
    let meta =
        fs::metadata(file_path).map_err(|_| AppErrorDto::io_error("Cannot read audio file"))?;
    if !meta.is_file() {
        return Err(AppErrorDto::io_error("Path is not a file"));
    }
    if meta.len() > ASYNC_MAX_AUDIO_BYTES {
        return Err(AppErrorDto::asr_payload_too_large(ASYNC_MAX_AUDIO_BYTES));
    }
    Ok(file_path)
}

fn title_from_path(file_path: &Path) -> Option<String> {
    file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
}

/// Create a draft meeting with no audio yet (`source_path` empty).
pub fn create_draft(conn: &Connection) -> CmdResult<Meeting> {
    let meeting = Meeting {
        id: Uuid::new_v4().to_string(),
        source_path: String::new(),
        title: Some(DEFAULT_DRAFT_TITLE.to_string()),
        created_at: Utc::now().to_rfc3339(),
    };

    conn.execute(
        "INSERT INTO meetings (id, source_path, title, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![
            meeting.id,
            meeting.source_path,
            meeting.title,
            meeting.created_at
        ],
    )
    .map_err(AppErrorDto::from)?;

    Ok(meeting)
}

pub fn create_from_file(conn: &Connection, path: &str) -> CmdResult<Meeting> {
    let file_path = validate_audio_path(path)?;
    let path = path.trim();
    let title = title_from_path(file_path);

    let meeting = Meeting {
        id: Uuid::new_v4().to_string(),
        source_path: path.to_string(),
        title,
        created_at: Utc::now().to_rfc3339(),
    };

    conn.execute(
        "INSERT INTO meetings (id, source_path, title, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![
            meeting.id,
            meeting.source_path,
            meeting.title,
            meeting.created_at
        ],
    )
    .map_err(AppErrorDto::from)?;

    Ok(meeting)
}

/// Attach an audio file to an existing draft (`source_path` must be empty).
pub fn attach_source(conn: &Connection, meeting_id: &str, path: &str) -> CmdResult<Meeting> {
    let file_path = validate_audio_path(path)?;
    let path = path.trim();
    let meeting = get_meeting(conn, meeting_id)?;
    if !meeting.source_path.trim().is_empty() {
        return Err(AppErrorDto::invalid_argument(
            "Meeting already has a source file",
        ));
    }

    let keep_title = meeting
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty() && *t != DEFAULT_DRAFT_TITLE)
        .map(|t| t.to_string());
    let title = keep_title.or_else(|| title_from_path(file_path));

    conn.execute(
        "UPDATE meetings SET source_path = ?1, title = ?2 WHERE id = ?3",
        rusqlite::params![path, title, meeting_id],
    )
    .map_err(AppErrorDto::from)?;

    get_meeting(conn, meeting_id)
}

pub fn list_meetings(conn: &Connection) -> CmdResult<Vec<Meeting>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, source_path, title, created_at FROM meetings
             ORDER BY created_at DESC",
        )
        .map_err(AppErrorDto::from)?;

    let rows = stmt
        .query_map([], |row| {
            Ok(Meeting {
                id: row.get(0)?,
                source_path: row.get(1)?,
                title: row.get(2)?,
                created_at: row.get(3)?,
            })
        })
        .map_err(AppErrorDto::from)?;

    let mut meetings = Vec::new();
    for row in rows {
        meetings.push(row.map_err(AppErrorDto::from)?);
    }
    Ok(meetings)
}

pub fn get_meeting(conn: &Connection, meeting_id: &str) -> CmdResult<Meeting> {
    let mut stmt = conn
        .prepare("SELECT id, source_path, title, created_at FROM meetings WHERE id = ?1")
        .map_err(AppErrorDto::from)?;

    stmt.query_row(rusqlite::params![meeting_id], |row| {
        Ok(Meeting {
            id: row.get(0)?,
            source_path: row.get(1)?,
            title: row.get(2)?,
            created_at: row.get(3)?,
        })
    })
    .map_err(|err| match err {
        rusqlite::Error::QueryReturnedNoRows => AppErrorDto::not_found("Meeting not found"),
        other => AppErrorDto::from(other),
    })
}

pub fn rename_meeting(conn: &Connection, meeting_id: &str, title: &str) -> CmdResult<Meeting> {
    let title = title.trim();
    if title.is_empty() {
        return Err(AppErrorDto::invalid_argument(
            "Meeting title cannot be empty",
        ));
    }
    let _ = get_meeting(conn, meeting_id)?;
    conn.execute(
        "UPDATE meetings SET title = ?1 WHERE id = ?2",
        rusqlite::params![title, meeting_id],
    )
    .map_err(AppErrorDto::from)?;
    get_meeting(conn, meeting_id)
}

/// Hard-delete meeting and related rows. Does **not** delete the audio file on disk.
pub fn delete_meeting(conn: &Connection, meeting_id: &str) -> CmdResult<()> {
    let _ = get_meeting(conn, meeting_id)?;
    conn.execute(
        "DELETE FROM summaries WHERE meeting_id = ?1",
        rusqlite::params![meeting_id],
    )
    .map_err(AppErrorDto::from)?;
    conn.execute(
        "DELETE FROM transcripts WHERE meeting_id = ?1",
        rusqlite::params![meeting_id],
    )
    .map_err(AppErrorDto::from)?;
    conn.execute(
        "DELETE FROM jobs WHERE meeting_id = ?1",
        rusqlite::params![meeting_id],
    )
    .map_err(AppErrorDto::from)?;
    conn.execute(
        "DELETE FROM meetings WHERE id = ?1",
        rusqlite::params![meeting_id],
    )
    .map_err(AppErrorDto::from)?;
    Ok(())
}

fn decode_segments(json: Option<String>) -> CmdResult<Vec<TranscriptSegment>> {
    match json {
        None => Ok(vec![]),
        Some(s) if s.trim().is_empty() => Ok(vec![]),
        Some(s) => serde_json::from_str(&s).map_err(AppErrorDto::from),
    }
}

fn decode_speaker_names(json: Option<String>) -> CmdResult<BTreeMap<String, String>> {
    match json {
        None => Ok(BTreeMap::new()),
        Some(s) if s.trim().is_empty() => Ok(BTreeMap::new()),
        Some(s) => serde_json::from_str(&s).map_err(AppErrorDto::from),
    }
}

pub fn get_transcript(conn: &Connection, meeting_id: &str) -> CmdResult<Transcript> {
    let _ = get_meeting(conn, meeting_id)?;

    let mut stmt = conn
        .prepare(
            "SELECT meeting_id, text, segments_json, speaker_names_json
             FROM transcripts WHERE meeting_id = ?1",
        )
        .map_err(AppErrorDto::from)?;

    stmt.query_row(rusqlite::params![meeting_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
        ))
    })
    .map_err(|err| match err {
        rusqlite::Error::QueryReturnedNoRows => AppErrorDto::not_found("Transcript not found"),
        other => AppErrorDto::from(other),
    })
    .and_then(|(meeting_id, text, segments_json, names_json)| {
        Ok(Transcript {
            meeting_id,
            text,
            segments: decode_segments(segments_json)?,
            speaker_names: decode_speaker_names(names_json)?,
        })
    })
}

/// Persist ASR output: parse speakers from raw JSON when present.
pub fn upsert_transcript_from_asr(
    conn: &Connection,
    meeting_id: &str,
    fallback_text: &str,
    raw_json: Option<&str>,
) -> CmdResult<()> {
    let parsed = match raw_json {
        Some(raw) => parse_asr_transcript(raw, fallback_text),
        None => crate::providers::doubao::ParsedAsrTranscript {
            text: fallback_text.to_string(),
            segments: vec![],
            speaker_names: BTreeMap::new(),
        },
    };
    upsert_transcript_parts(
        conn,
        meeting_id,
        &parsed.text,
        raw_json,
        &parsed.segments,
        &parsed.speaker_names,
    )
}

/// Plain upsert used by tests that already have final text (no ASR parse).
#[allow(dead_code)]
pub fn upsert_transcript(
    conn: &Connection,
    meeting_id: &str,
    text: &str,
    raw_json: Option<&str>,
) -> CmdResult<()> {
    upsert_transcript_parts(conn, meeting_id, text, raw_json, &[], &BTreeMap::new())
}

fn upsert_transcript_parts(
    conn: &Connection,
    meeting_id: &str,
    text: &str,
    raw_json: Option<&str>,
    segments: &[TranscriptSegment],
    speaker_names: &BTreeMap<String, String>,
) -> CmdResult<()> {
    let segments_json = serde_json::to_string(segments).map_err(AppErrorDto::from)?;
    let names_json = serde_json::to_string(speaker_names).map_err(AppErrorDto::from)?;
    conn.execute(
        "INSERT INTO transcripts
         (meeting_id, text, raw_json, segments_json, speaker_names_json)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(meeting_id) DO UPDATE SET
           text = excluded.text,
           raw_json = excluded.raw_json,
           segments_json = excluded.segments_json,
           speaker_names_json = excluded.speaker_names_json",
        rusqlite::params![meeting_id, text, raw_json, segments_json, names_json],
    )
    .map_err(AppErrorDto::from)?;
    Ok(())
}

fn delete_summary_for_meeting(conn: &Connection, meeting_id: &str) -> CmdResult<()> {
    conn.execute(
        "DELETE FROM summaries WHERE meeting_id = ?1",
        rusqlite::params![meeting_id],
    )
    .map_err(AppErrorDto::from)?;
    Ok(())
}

/// Apply speaker display names, re-render transcript text, invalidate summary.
pub fn update_speakers(
    conn: &Connection,
    meeting_id: &str,
    speaker_names: BTreeMap<String, String>,
) -> CmdResult<Transcript> {
    let existing = get_transcript(conn, meeting_id)?;
    if existing.segments.is_empty() {
        return Err(AppErrorDto::transcript_no_speakers());
    }

    let mut names = existing.speaker_names;
    for (id, name) in speaker_names {
        let trimmed = name.trim().to_string();
        if trimmed.is_empty() {
            return Err(AppErrorDto::invalid_argument(
                "Speaker display name cannot be empty",
            ));
        }
        if !existing.segments.iter().any(|s| s.speaker_id == id) {
            return Err(AppErrorDto::invalid_argument(format!(
                "Unknown speaker id: {id}"
            )));
        }
        names.insert(id, trimmed);
    }

    let text = render_transcript_text(&existing.segments, &names);
    // Preserve raw_json: read and write back via UPDATE of known columns only.
    let raw_json: Option<String> = conn
        .query_row(
            "SELECT raw_json FROM transcripts WHERE meeting_id = ?1",
            rusqlite::params![meeting_id],
            |row| row.get(0),
        )
        .map_err(AppErrorDto::from)?;

    upsert_transcript_parts(
        conn,
        meeting_id,
        &text,
        raw_json.as_deref(),
        &existing.segments,
        &names,
    )?;
    delete_summary_for_meeting(conn, meeting_id)?;
    get_transcript(conn, meeting_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool::open_memory;
    use std::io::Write;

    #[test]
    fn create_and_get_meeting() {
        let conn = open_memory().expect("db");
        let dir = std::env::temp_dir();
        let path = dir.join(format!("meetphant-test-{}.wav", Uuid::new_v4()));
        {
            let mut f = fs::File::create(&path).expect("create");
            f.write_all(b"RIFF").expect("write");
        }
        let meeting = create_from_file(&conn, path.to_str().unwrap()).expect("create");
        let loaded = get_meeting(&conn, &meeting.id).expect("get");
        assert_eq!(loaded.id, meeting.id);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn create_draft_and_attach_source() {
        let conn = open_memory().expect("db");
        let draft = create_draft(&conn).expect("draft");
        assert_eq!(draft.title.as_deref(), Some(DEFAULT_DRAFT_TITLE));
        assert!(draft.source_path.is_empty());

        let dir = std::env::temp_dir();
        let path = dir.join(format!("meetphant-attach-{}.wav", Uuid::new_v4()));
        {
            let mut f = fs::File::create(&path).expect("create");
            f.write_all(b"RIFF").expect("write");
        }
        let attached = attach_source(&conn, &draft.id, path.to_str().unwrap()).expect("attach");
        assert_eq!(attached.id, draft.id);
        assert_eq!(attached.source_path, path.to_str().unwrap());
        assert_eq!(
            attached.title.as_deref(),
            path.file_stem().and_then(|s| s.to_str())
        );

        let err =
            attach_source(&conn, &draft.id, path.to_str().unwrap()).expect_err("second attach");
        assert_eq!(err.code, "INVALID_ARGUMENT");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn attach_source_keeps_renamed_title() {
        let conn = open_memory().expect("db");
        let draft = create_draft(&conn).expect("draft");
        rename_meeting(&conn, &draft.id, "周会").unwrap();

        let dir = std::env::temp_dir();
        let path = dir.join(format!("meetphant-keep-title-{}.wav", Uuid::new_v4()));
        {
            let mut f = fs::File::create(&path).expect("create");
            f.write_all(b"RIFF").expect("write");
        }
        let attached = attach_source(&conn, &draft.id, path.to_str().unwrap()).expect("attach");
        assert_eq!(attached.title.as_deref(), Some("周会"));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn list_rename_delete_meeting() {
        let conn = open_memory().expect("db");
        let dir = std::env::temp_dir();
        let path = dir.join(format!("meetphant-list-{}.wav", Uuid::new_v4()));
        {
            let mut f = fs::File::create(&path).expect("create");
            f.write_all(b"RIFF").expect("write");
        }
        let meeting = create_from_file(&conn, path.to_str().unwrap()).expect("create");
        upsert_transcript_from_asr(
            &conn,
            &meeting.id,
            "fallback",
            Some(
                r#"{"result":{"text":"ab","utterances":[
                  {"text":"a","additions":{"speaker":"1"}},
                  {"text":"b","additions":{"speaker":"2"}}
                ]}}"#,
            ),
        )
        .unwrap();

        let listed = list_meetings(&conn).unwrap();
        assert_eq!(listed.len(), 1);

        let renamed = rename_meeting(&conn, &meeting.id, "  周会  ").unwrap();
        assert_eq!(renamed.title.as_deref(), Some("周会"));

        let mut names = BTreeMap::new();
        names.insert("1".into(), "张三".into());
        let updated = update_speakers(&conn, &meeting.id, names).unwrap();
        assert!(updated.text.contains("【张三】"));
        assert!(updated.text.contains("【发言人2】"));

        delete_meeting(&conn, &meeting.id).unwrap();
        assert!(list_meetings(&conn).unwrap().is_empty());
        assert!(path.exists());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn rejects_oversized_file() {
        let conn = open_memory().expect("db");
        let dir = std::env::temp_dir();
        let path = dir.join(format!("meetphant-big-{}.wav", Uuid::new_v4()));
        {
            let f = fs::File::create(&path).expect("create");
            f.set_len(ASYNC_MAX_AUDIO_BYTES + 1).expect("size");
        }
        let err = create_from_file(&conn, path.to_str().unwrap()).expect_err("too big");
        assert_eq!(err.code, "ASR_PAYLOAD_TOO_LARGE");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn accepts_large_file_under_async_cap() {
        let conn = open_memory().expect("db");
        let dir = std::env::temp_dir();
        let path = dir.join(format!("meetphant-mid-{}.wav", Uuid::new_v4()));
        {
            let f = fs::File::create(&path).expect("create");
            f.set_len(20 * 1024 * 1024 + 1).expect("size");
        }
        let meeting = create_from_file(&conn, path.to_str().unwrap()).expect("create");
        assert!(!meeting.id.is_empty());
        let _ = fs::remove_file(&path);
    }
}
