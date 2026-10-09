use std::sync::Mutex;

use chrono::Utc;
use rusqlite::Connection;

use crate::db;
use crate::error::{AppErrorDto, CmdResult};
use crate::models::{is_supported_summary_language, Settings, Summary};
use crate::providers::openai_compat::{
    HttpChatClient, LlmConfig, SummaryGenerateInput, SummaryGenerator,
};
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

/// Effective summary LLM config from settings + keyring. Empty base URL or
/// model (e.g. an unknown provider id in SQLite) counts as not configured.
fn require_llm_config(settings: &Settings) -> CmdResult<LlmConfig> {
    let api_key = credentials::require_summary_llm_credentials()?.api_key;
    if settings.summary_llm_base_url.trim().is_empty()
        || settings.summary_llm_model.trim().is_empty()
    {
        return Err(AppErrorDto::new(
            "SUMMARY_NOT_CONFIGURED",
            "摘要模型服务未完整配置（Base URL / 模型），请先在设置中配置",
        ));
    }
    Ok(LlmConfig {
        provider: settings.summary_llm_provider.clone(),
        base_url: settings.summary_llm_base_url.clone(),
        model: settings.summary_llm_model.clone(),
        api_key,
    })
}

/// Generate (or regenerate) a summary for a meeting that already has a transcript.
///
/// The DB lock is held only to read inputs and to write the result, never while
/// `generator` runs (an LLM call can take up to the HTTP timeout).
pub fn generate_summary(
    db: &Mutex<Connection>,
    meeting_id: &str,
    language: &str,
    generator: &dyn SummaryGenerator,
) -> CmdResult<Summary> {
    if !is_supported_summary_language(language) {
        return Err(AppErrorDto::invalid_argument(
            "Unsupported summary language; use zh-CN, en, or zh-en",
        ));
    }
    let (transcript, settings) = {
        let conn = db::lock(db)?;
        let _ = meeting_service::get_meeting(&conn, meeting_id)?;
        let transcript = require_transcript(&conn, meeting_id)?;
        (transcript, settings_service::get_settings(&conn)?)
    };
    let config = require_llm_config(&settings)?;
    let context_text = settings.context_text;

    let content = generator.generate(
        &config,
        &SummaryGenerateInput {
            transcript,
            context_text,
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
    let conn = db::lock(db)?;
    // The meeting may have been deleted while the lock was released. Foreign keys are
    // not enforced (`PRAGMA foreign_keys` is off), so check before writing an orphan.
    let _ = meeting_service::get_meeting(&conn, meeting_id)?;
    upsert_summary(&conn, &summary)?;
    Ok(summary)
}

/// Production entry: real OpenAI-compatible HTTP client.
pub fn generate_summary_http(
    db: &Mutex<Connection>,
    meeting_id: &str,
    language: &str,
) -> CmdResult<Summary> {
    let client = HttpChatClient::new()?;
    generate_summary(db, meeting_id, language, &client)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool::open_memory;
    use crate::models::{SettingsUpdate, SummaryContent};
    use crate::services::credentials::{
        reset_for_test, set_credentials, set_summary_llm_credentials,
    };
    use crate::services::meeting_service::{create_from_file, upsert_transcript};
    use crate::services::settings_service::update_settings;
    use std::io::Write;
    use std::sync::Mutex;
    use uuid::Uuid;

    struct StubGenerator {
        result: Mutex<Result<SummaryContent, AppErrorDto>>,
        last_context: Mutex<Option<String>>,
        last_config: Mutex<Option<LlmConfig>>,
    }

    impl StubGenerator {
        fn ok(content: SummaryContent) -> Self {
            Self {
                result: Mutex::new(Ok(content)),
                last_context: Mutex::new(None),
                last_config: Mutex::new(None),
            }
        }
    }

    impl SummaryGenerator for StubGenerator {
        fn generate(
            &self,
            config: &LlmConfig,
            input: &SummaryGenerateInput,
        ) -> CmdResult<SummaryContent> {
            *self.last_context.lock().unwrap() = Some(input.context_text.clone());
            *self.last_config.lock().unwrap() = Some(config.clone());
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
        set_summary_llm_credentials("sk-test").unwrap();
        let db = Mutex::new(open_memory().unwrap());
        let (meeting_id, path) = seed_meeting_with_transcript(&db.lock().unwrap());

        let stub = StubGenerator::ok(SummaryContent {
            key_points: vec!["发布计划".into()],
            action_items: vec![],
            decisions: vec!["下周上线".into()],
        });

        let summary = generate_summary(&db, &meeting_id, "zh-CN", &stub).unwrap();
        assert_eq!(summary.language, "zh-CN");
        assert_eq!(summary.key_points, vec!["发布计划"]);
        assert_eq!(summary.decisions, vec!["下周上线"]);
        assert_eq!(stub.last_context.lock().unwrap().as_deref(), Some(""));

        let loaded = get_summary(&db.lock().unwrap(), &meeting_id).unwrap();
        assert_eq!(loaded.key_points, summary.key_points);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn generate_includes_context_text() {
        reset_for_test();
        set_summary_llm_credentials("sk-test").unwrap();
        let db = Mutex::new(open_memory().unwrap());
        let (meeting_id, path) = seed_meeting_with_transcript(&db.lock().unwrap());
        update_settings(
            &db.lock().unwrap(),
            SettingsUpdate {
                context_text: Some("产品周会".into()),
                ..Default::default()
            },
        )
        .unwrap();

        let stub = StubGenerator::ok(SummaryContent::default());
        generate_summary(&db, &meeting_id, "zh-CN", &stub).unwrap();
        assert_eq!(
            stub.last_context.lock().unwrap().as_deref(),
            Some("产品周会")
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn not_ready_without_transcript() {
        reset_for_test();
        set_summary_llm_credentials("sk-test").unwrap();
        let db = Mutex::new(open_memory().unwrap());
        set_credentials("doubao-key").unwrap();
        let (path, path_str) = temp_audio();
        let meeting = create_from_file(&db.lock().unwrap(), &path_str).unwrap();

        let stub = StubGenerator::ok(SummaryContent::default());
        let err = generate_summary(&db, &meeting.id, "zh-CN", &stub).expect_err("not ready");
        assert_eq!(err.code, "SUMMARY_NOT_READY");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn upgraded_install_uses_dashscope_defaults() {
        // AC1: legacy DashScope key + untouched settings → DashScope + qwen3.7-plus.
        reset_for_test();
        credentials::seed_legacy_dashscope_key_for_test("sk-legacy");
        credentials::migrate_legacy_credentials();
        let db = Mutex::new(open_memory().unwrap());
        let (meeting_id, path) = seed_meeting_with_transcript(&db.lock().unwrap());

        let stub = StubGenerator::ok(SummaryContent::default());
        generate_summary(&db, &meeting_id, "zh-CN", &stub).unwrap();
        let config = stub.last_config.lock().unwrap().clone().unwrap();
        assert_eq!(config.provider, "dashscope");
        assert_eq!(
            config.chat_completions_url(),
            "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions"
        );
        assert_eq!(config.model, "qwen3.7-plus");
        assert_eq!(config.api_key, "sk-legacy");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn generate_uses_configured_provider() {
        // AC2: DeepSeek + key + model from settings reach the generator.
        reset_for_test();
        let db = Mutex::new(open_memory().unwrap());
        let (meeting_id, path) = seed_meeting_with_transcript(&db.lock().unwrap());
        update_settings(
            &db.lock().unwrap(),
            SettingsUpdate {
                summary_llm_provider: Some("deepseek".into()),
                summary_llm_base_url: Some("https://api.deepseek.com/v1/".into()),
                summary_llm_model: Some("deepseek-reasoner".into()),
                summary_llm_api_key: Some("sk-deepseek".into()),
                ..Default::default()
            },
        )
        .unwrap();

        let stub = StubGenerator::ok(SummaryContent::default());
        generate_summary(&db, &meeting_id, "zh-CN", &stub).unwrap();
        let config = stub.last_config.lock().unwrap().clone().unwrap();
        assert_eq!(config.provider, "deepseek");
        assert_eq!(
            config.chat_completions_url(),
            "https://api.deepseek.com/v1/chat/completions"
        );
        assert_eq!(config.model, "deepseek-reasoner");
        assert_eq!(config.api_key, "sk-deepseek");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn not_configured_without_summary_llm_key() {
        reset_for_test();
        let db = Mutex::new(open_memory().unwrap());
        let (meeting_id, path) = seed_meeting_with_transcript(&db.lock().unwrap());

        let stub = StubGenerator::ok(SummaryContent::default());
        let err = generate_summary(&db, &meeting_id, "zh-CN", &stub).expect_err("no key");
        assert_eq!(err.code, "SUMMARY_NOT_CONFIGURED");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn parse_failure_from_provider() {
        reset_for_test();
        set_summary_llm_credentials("sk-test").unwrap();
        let db = Mutex::new(open_memory().unwrap());
        let (meeting_id, path) = seed_meeting_with_transcript(&db.lock().unwrap());

        let stub = StubGenerator {
            result: Mutex::new(Err(AppErrorDto::summary_provider_error(
                "Invalid summary JSON from provider",
            ))),
            last_context: Mutex::new(None),
            last_config: Mutex::new(None),
        };
        let err = generate_summary(&db, &meeting_id, "zh-CN", &stub).expect_err("parse");
        assert_eq!(err.code, "SUMMARY_PROVIDER_ERROR");
        let missing = get_summary(&db.lock().unwrap(), &meeting_id).expect_err("no row");
        assert_eq!(missing.code, "NOT_FOUND");
        let _ = std::fs::remove_file(path);
    }

    /// Fails the generation if the DB lock is held while the provider runs.
    struct LockProbeGenerator<'a> {
        db: &'a Mutex<Connection>,
    }

    impl SummaryGenerator for LockProbeGenerator<'_> {
        fn generate(
            &self,
            _config: &LlmConfig,
            _input: &SummaryGenerateInput,
        ) -> CmdResult<SummaryContent> {
            assert!(
                self.db.try_lock().is_ok(),
                "DB lock must not be held while the summary provider runs"
            );
            Ok(SummaryContent::default())
        }
    }

    #[test]
    fn db_lock_released_during_generation() {
        reset_for_test();
        set_summary_llm_credentials("sk-test").unwrap();
        let db = Mutex::new(open_memory().unwrap());
        let (meeting_id, path) = seed_meeting_with_transcript(&db.lock().unwrap());

        let probe = LockProbeGenerator { db: &db };
        generate_summary(&db, &meeting_id, "zh-CN", &probe).unwrap();
        assert!(get_summary(&db.lock().unwrap(), &meeting_id).is_ok());
        let _ = std::fs::remove_file(path);
    }

    /// Deletes the meeting mid-generation, as a concurrent `meetings_delete` could.
    struct DeletingGenerator<'a> {
        db: &'a Mutex<Connection>,
        meeting_id: String,
    }

    impl SummaryGenerator for DeletingGenerator<'_> {
        fn generate(
            &self,
            _config: &LlmConfig,
            _input: &SummaryGenerateInput,
        ) -> CmdResult<SummaryContent> {
            meeting_service::delete_meeting(&self.db.lock().unwrap(), &self.meeting_id)?;
            Ok(SummaryContent::default())
        }
    }

    #[test]
    fn meeting_deleted_during_generation_writes_nothing() {
        reset_for_test();
        set_summary_llm_credentials("sk-test").unwrap();
        let db = Mutex::new(open_memory().unwrap());
        let (meeting_id, path) = seed_meeting_with_transcript(&db.lock().unwrap());

        let deleting = DeletingGenerator {
            db: &db,
            meeting_id: meeting_id.clone(),
        };
        let err = generate_summary(&db, &meeting_id, "zh-CN", &deleting).expect_err("deleted");
        assert_eq!(err.code, "NOT_FOUND");
        let rows: i64 = db
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM summaries", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 0);
        let _ = std::fs::remove_file(path);
    }
}
