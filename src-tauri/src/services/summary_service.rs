use chrono::Utc;
use rusqlite::Connection;

use crate::error::{AppErrorDto, CmdResult};
use crate::models::{is_supported_summary_language, Summary};
use crate::providers::qwen::{HttpQwenClient, SummaryGenerateInput, SummaryGenerator};
use crate::services::{credentials, meeting_service, settings_service};

fn encode_list(items: &[String]) -> CmdResult<String> {
    serde_json::to_string(items).map_err(AppErrorDto::from)
}

fn decode_list(json: &str) -> CmdResult<Vec<String>> {
    serde_json::from_str(json).map_err(AppErrorDto::from)
}

fn row_to_summary(
    meeting_id: String,
    key_points_json: String,
    action_items_json: String,
    decisions_json: String,
    language: String,
    created_at: String,
) -> CmdResult<Summary> {
    Ok(Summary {
        meeting_id,
        key_points: decode_list(&key_points_json)?,
        action_items: decode_list(&action_items_json)?,
        decisions: decode_list(&decisions_json)?,
        language,
        created_at,
    })
}

pub fn get_summary(conn: &Connection, meeting_id: &str) -> CmdResult<Summary> {
    let _ = meeting_service::get_meeting(conn, meeting_id)?;

    let mut stmt = conn
        .prepare(
            "SELECT meeting_id, key_points, action_items, decisions, language, created_at
             FROM summaries WHERE meeting_id = ?1",
        )
        .map_err(AppErrorDto::from)?;

    stmt.query_row(rusqlite::params![meeting_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
        ))
    })
    .map_err(|err| match err {
        rusqlite::Error::QueryReturnedNoRows => AppErrorDto::not_found("Summary not found"),
        other => AppErrorDto::from(other),
    })
    .and_then(|(meeting_id, kp, ai, dec, lang, created)| {
        row_to_summary(meeting_id, kp, ai, dec, lang, created)
    })
}

fn upsert_summary(conn: &Connection, summary: &Summary) -> CmdResult<()> {
    conn.execute(
        "INSERT INTO summaries
         (meeting_id, key_points, action_items, decisions, language, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(meeting_id) DO UPDATE SET
           key_points = excluded.key_points,
           action_items = excluded.action_items,
           decisions = excluded.decisions,
           language = excluded.language,
           created_at = excluded.created_at",
        rusqlite::params![
            summary.meeting_id,
            encode_list(&summary.key_points)?,
            encode_list(&summary.action_items)?,
            encode_list(&summary.decisions)?,
            summary.language,
            summary.created_at,
        ],
    )
    .map_err(AppErrorDto::from)?;
    Ok(())
}

fn require_transcript(conn: &Connection, meeting_id: &str) -> CmdResult<String> {
    match meeting_service::get_transcript(conn, meeting_id) {
        Ok(t) => Ok(t.text),
        Err(err) if err.code == "NOT_FOUND" => Err(AppErrorDto::summary_not_ready()),
        Err(err) => Err(err),
    }
}

/// Generate (or regenerate) a summary for a meeting that already has a transcript.
pub fn generate_summary(
    conn: &Connection,
    meeting_id: &str,
    language: &str,
    generator: &dyn SummaryGenerator,
) -> CmdResult<Summary> {
    if !is_supported_summary_language(language) {
        return Err(AppErrorDto::invalid_argument(
            "Unsupported summary language; use zh-CN, en, or zh-en",
        ));
    }
    let _ = meeting_service::get_meeting(conn, meeting_id)?;
    let transcript = require_transcript(conn, meeting_id)?;
    let settings = settings_service::get_settings(conn)?;
    let credentials = credentials::require_dashscope_credentials()?;

    let content = generator.generate(
        &credentials,
        &SummaryGenerateInput {
            transcript,
            context_text: settings.context_text,
            language: language.to_string(),
        },
    )?;

    let summary = Summary {
        meeting_id: meeting_id.to_string(),
        key_points: content.key_points,
        action_items: content.action_items,
        decisions: content.decisions,
        language: language.to_string(),
        created_at: Utc::now().to_rfc3339(),
    };
    upsert_summary(conn, &summary)?;
    Ok(summary)
}

/// Production entry: real HTTP Qwen client.
pub fn generate_summary_http(
    conn: &Connection,
    meeting_id: &str,
    language: &str,
) -> CmdResult<Summary> {
    let client = HttpQwenClient::new()?;
    generate_summary(conn, meeting_id, language, &client)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool::open_memory;
    use crate::models::{SettingsUpdate, SummaryContent};
    use crate::providers::qwen::SummaryGenerateInput;
    use crate::services::credentials::{
        reset_for_test, set_credentials, set_dashscope_credentials,
    };
    use crate::services::meeting_service::{create_from_file, upsert_transcript};
    use crate::services::settings_service::update_settings;
    use std::io::Write;
    use std::sync::Mutex;
    use uuid::Uuid;

    struct StubGenerator {
        result: Mutex<Result<SummaryContent, AppErrorDto>>,
        last_context: Mutex<Option<String>>,
    }

    impl SummaryGenerator for StubGenerator {
        fn generate(
            &self,
            _credentials: &credentials::DashScopeCredentials,
            input: &SummaryGenerateInput,
        ) -> CmdResult<SummaryContent> {
            *self.last_context.lock().unwrap() = Some(input.context_text.clone());
            match &*self.result.lock().unwrap() {
                Ok(out) => Ok(out.clone()),
                Err(err) => Err(err.clone()),
            }
        }
    }

    fn temp_audio() -> (std::path::PathBuf, String) {
        let path = std::env::temp_dir().join(format!("meetphant-sum-{}.wav", Uuid::new_v4()));
        let mut f = std::fs::File::create(&path).expect("create");
        f.write_all(b"fake-audio").expect("write");
        let s = path.to_str().unwrap().to_string();
        (path, s)
    }

    fn seed_meeting_with_transcript(conn: &Connection) -> (String, std::path::PathBuf) {
        set_credentials("doubao-key").unwrap();
        let (path, path_str) = temp_audio();
        let meeting = create_from_file(conn, &path_str).unwrap();
        upsert_transcript(conn, &meeting.id, "会议讨论了发布计划", None).unwrap();
        (meeting.id, path)
    }

    #[test]
    fn generate_with_empty_context_works() {
        reset_for_test();
        set_dashscope_credentials("sk-test").unwrap();
        let conn = open_memory().unwrap();
        let (meeting_id, path) = seed_meeting_with_transcript(&conn);

        let stub = StubGenerator {
            result: Mutex::new(Ok(SummaryContent {
                key_points: vec!["发布计划".into()],
                action_items: vec![],
                decisions: vec!["下周上线".into()],
            })),
            last_context: Mutex::new(None),
        };

        let summary = generate_summary(&conn, &meeting_id, "zh-CN", &stub).unwrap();
        assert_eq!(summary.language, "zh-CN");
        assert_eq!(summary.key_points, vec!["发布计划"]);
        assert_eq!(summary.decisions, vec!["下周上线"]);
        assert_eq!(stub.last_context.lock().unwrap().as_deref(), Some(""));

        let loaded = get_summary(&conn, &meeting_id).unwrap();
        assert_eq!(loaded.key_points, summary.key_points);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn generate_includes_context_text() {
        reset_for_test();
        set_dashscope_credentials("sk-test").unwrap();
        let conn = open_memory().unwrap();
        let (meeting_id, path) = seed_meeting_with_transcript(&conn);
        update_settings(
            &conn,
            SettingsUpdate {
                context_text: Some("产品周会".into()),
                ..Default::default()
            },
        )
        .unwrap();

        let stub = StubGenerator {
            result: Mutex::new(Ok(SummaryContent::default())),
            last_context: Mutex::new(None),
        };
        generate_summary(&conn, &meeting_id, "zh-CN", &stub).unwrap();
        assert_eq!(
            stub.last_context.lock().unwrap().as_deref(),
            Some("产品周会")
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn not_ready_without_transcript() {
        reset_for_test();
        set_dashscope_credentials("sk-test").unwrap();
        let conn = open_memory().unwrap();
        set_credentials("doubao-key").unwrap();
        let (path, path_str) = temp_audio();
        let meeting = create_from_file(&conn, &path_str).unwrap();

        let stub = StubGenerator {
            result: Mutex::new(Ok(SummaryContent::default())),
            last_context: Mutex::new(None),
        };
        let err = generate_summary(&conn, &meeting.id, "zh-CN", &stub).expect_err("not ready");
        assert_eq!(err.code, "SUMMARY_NOT_READY");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn not_configured_without_dashscope_key() {
        reset_for_test();
        let conn = open_memory().unwrap();
        let (meeting_id, path) = seed_meeting_with_transcript(&conn);

        let stub = StubGenerator {
            result: Mutex::new(Ok(SummaryContent::default())),
            last_context: Mutex::new(None),
        };
        let err = generate_summary(&conn, &meeting_id, "zh-CN", &stub).expect_err("no key");
        assert_eq!(err.code, "SUMMARY_NOT_CONFIGURED");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn parse_failure_from_provider() {
        reset_for_test();
        set_dashscope_credentials("sk-test").unwrap();
        let conn = open_memory().unwrap();
        let (meeting_id, path) = seed_meeting_with_transcript(&conn);

        let stub = StubGenerator {
            result: Mutex::new(Err(AppErrorDto::summary_provider_error(
                "Invalid summary JSON from provider",
            ))),
            last_context: Mutex::new(None),
        };
        let err = generate_summary(&conn, &meeting_id, "zh-CN", &stub).expect_err("parse");
        assert_eq!(err.code, "SUMMARY_PROVIDER_ERROR");
        let missing = get_summary(&conn, &meeting_id).expect_err("no row");
        assert_eq!(missing.code, "NOT_FOUND");
        let _ = std::fs::remove_file(path);
    }
}
