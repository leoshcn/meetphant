use std::fs;
use std::time::{Duration, Instant};

use chrono::Utc;
use rusqlite::Connection;
use uuid::Uuid;

use crate::error::{AppErrorDto, CmdResult};
use crate::models::{
    Job, JOB_KIND_TRANSCRIPTION, JOB_STATUS_FAILED, JOB_STATUS_RUNNING, JOB_STATUS_SUCCEEDED,
};
use crate::providers::doubao::{
    audio_format_from_path, poll_until_done, AsyncRecognizer, AsyncSubmitInput, HttpAsyncClient,
    ASYNC_POLL_INTERVAL, ASYNC_POLL_TIMEOUT,
};
use crate::providers::tos::{
    build_object_key, HttpTosClient, ObjectStorage, TosConfig, PRESIGN_TTL_SECS,
};
use crate::services::{credentials, meeting_service, settings_service};

pub fn get_job(conn: &Connection, job_id: &str) -> CmdResult<Job> {
    let mut stmt = conn
        .prepare(
            "SELECT id, meeting_id, kind, status, error_code, error_message, created_at, updated_at
             FROM jobs WHERE id = ?1",
        )
        .map_err(AppErrorDto::from)?;

    stmt.query_row(rusqlite::params![job_id], |row| {
        Ok(Job {
            id: row.get(0)?,
            meeting_id: row.get(1)?,
            kind: row.get(2)?,
            status: row.get(3)?,
            error_code: row.get(4)?,
            error_message: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        })
    })
    .map_err(|err| match err {
        rusqlite::Error::QueryReturnedNoRows => AppErrorDto::not_found("Job not found"),
        other => AppErrorDto::from(other),
    })
}

fn insert_job(conn: &Connection, job: &Job) -> CmdResult<()> {
    conn.execute(
        "INSERT INTO jobs
         (id, meeting_id, kind, status, provider_task_id, error_code, error_message, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, NULL, NULL, NULL, ?5, ?6)",
        rusqlite::params![
            job.id,
            job.meeting_id,
            job.kind,
            job.status,
            job.created_at,
            job.updated_at
        ],
    )
    .map_err(AppErrorDto::from)?;
    Ok(())
}

pub fn set_provider_task_id(
    conn: &Connection,
    job_id: &str,
    provider_task_id: &str,
) -> CmdResult<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE jobs SET provider_task_id = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![provider_task_id, now, job_id],
    )
    .map_err(AppErrorDto::from)?;
    Ok(())
}

pub fn mark_job_succeeded(conn: &Connection, job_id: &str) -> CmdResult<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE jobs SET status = ?1, error_code = NULL, error_message = NULL, updated_at = ?2
         WHERE id = ?3",
        rusqlite::params![JOB_STATUS_SUCCEEDED, now, job_id],
    )
    .map_err(AppErrorDto::from)?;
    Ok(())
}

pub fn mark_job_failed(
    conn: &Connection,
    job_id: &str,
    code: &str,
    message: &str,
) -> CmdResult<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE jobs SET status = ?1, error_code = ?2, error_message = ?3, updated_at = ?4
         WHERE id = ?5",
        rusqlite::params![JOB_STATUS_FAILED, code, message, now, job_id],
    )
    .map_err(AppErrorDto::from)?;
    Ok(())
}

fn load_tos_config(conn: &Connection) -> CmdResult<TosConfig> {
    let settings = settings_service::get_settings(conn)?;
    if !settings.tos_configured {
        return Err(AppErrorDto::tos_not_configured());
    }
    let credentials = credentials::require_tos_credentials()?;
    Ok(TosConfig::from_parts(
        credentials,
        settings.tos_region,
        settings.tos_bucket,
        settings.tos_endpoint,
    ))
}

/// Create a running job row. Caller schedules `execute_transcription_job`.
pub fn start_transcription_job(conn: &Connection, meeting_id: &str) -> CmdResult<Job> {
    let _creds = credentials::require_credentials()?;
    let meeting = meeting_service::get_meeting(conn, meeting_id)?;

    // Fail fast on size / TOS before enqueue. Seed-ASR 2.0 only accepts `audio.url`,
    // so every file goes through TOS regardless of size.
    let meta = fs::metadata(&meeting.source_path)
        .map_err(|_| AppErrorDto::io_error("Cannot read audio file"))?;
    if meta.len() > meeting_service::ASYNC_MAX_AUDIO_BYTES {
        return Err(AppErrorDto::asr_payload_too_large(
            meeting_service::ASYNC_MAX_AUDIO_BYTES,
        ));
    }
    if !settings_service::is_tos_configured(conn) {
        return Err(AppErrorDto::tos_not_configured());
    }

    let now = Utc::now().to_rfc3339();
    let job = Job {
        id: Uuid::new_v4().to_string(),
        meeting_id: meeting.id,
        kind: JOB_KIND_TRANSCRIPTION.to_string(),
        status: JOB_STATUS_RUNNING.to_string(),
        error_code: None,
        error_message: None,
        created_at: now.clone(),
        updated_at: now,
    };
    insert_job(conn, &job)?;
    Ok(job)
}

struct JobWorkContext {
    meeting_id: String,
    source_path: String,
    hotwords: Vec<String>,
    file_size: u64,
}

fn load_work_context(conn: &Connection, job_id: &str) -> CmdResult<JobWorkContext> {
    let job = get_job(conn, job_id)?;
    let meeting = meeting_service::get_meeting(conn, &job.meeting_id)?;
    let settings = settings_service::get_settings(conn)?;
    let meta = fs::metadata(&meeting.source_path)
        .map_err(|_| AppErrorDto::io_error("Cannot read audio file"))?;
    Ok(JobWorkContext {
        meeting_id: meeting.id,
        source_path: meeting.source_path,
        hotwords: settings.hotwords,
        file_size: meta.len(),
    })
}

#[allow(clippy::too_many_arguments)]
fn run_async_path(
    conn: &Connection,
    job_id: &str,
    credentials: &credentials::DoubaoCredentials,
    ctx: &JobWorkContext,
    async_asr: &dyn AsyncRecognizer,
    tos: &dyn ObjectStorage,
    poll_timeout: Duration,
    poll_interval: Duration,
) -> CmdResult<(String, String)> {
    let tos_config = load_tos_config(conn)?;
    let object_key = build_object_key(&ctx.meeting_id, &ctx.source_path);

    tos.put_file(&tos_config, &ctx.source_path, &object_key)?;

    let audio_url = tos.pre_sign_get(&tos_config, &object_key, PRESIGN_TTL_SECS)?;

    let format = audio_format_from_path(&ctx.source_path);
    let submit = async_asr.submit(
        credentials,
        &AsyncSubmitInput {
            audio_url,
            format,
            hotwords: ctx.hotwords.clone(),
        },
    )?;

    set_provider_task_id(conn, job_id, &submit.request_id)?;

    let output = poll_until_done(
        async_asr,
        credentials,
        &submit.request_id,
        submit.log_id.as_deref(),
        poll_timeout,
        poll_interval,
    )?;

    // Best-effort delete — must not fail a successful transcript.
    if let Err(err) = tos.delete_object(&tos_config, &object_key) {
        tracing::warn!(object_key = %object_key, code = %err.code, "tos.delete failed");
    }

    Ok((output.text, output.raw_json))
}

/// Run recognize for an existing running job (TOS upload + Seed-ASR 2.0).
#[cfg_attr(not(test), allow(dead_code))]
pub fn execute_transcription_job(
    conn: &Connection,
    job_id: &str,
    async_asr: &dyn AsyncRecognizer,
    tos: &dyn ObjectStorage,
) -> CmdResult<()> {
    execute_transcription_job_with_poll(
        conn,
        job_id,
        async_asr,
        tos,
        ASYNC_POLL_TIMEOUT,
        ASYNC_POLL_INTERVAL,
    )
}

fn execute_transcription_job_with_poll(
    conn: &Connection,
    job_id: &str,
    async_asr: &dyn AsyncRecognizer,
    tos: &dyn ObjectStorage,
    poll_timeout: Duration,
    poll_interval: Duration,
) -> CmdResult<()> {
    let ctx = load_work_context(conn, job_id)?;
    let credentials = credentials::require_credentials()?;

    let result = (|| -> CmdResult<()> {
        if ctx.file_size > meeting_service::ASYNC_MAX_AUDIO_BYTES {
            return Err(AppErrorDto::asr_payload_too_large(
                meeting_service::ASYNC_MAX_AUDIO_BYTES,
            ));
        }

        let (text, raw_json) = run_async_path(
            conn,
            job_id,
            &credentials,
            &ctx,
            async_asr,
            tos,
            poll_timeout,
            poll_interval,
        )?;

        meeting_service::upsert_transcript_from_asr(conn, &ctx.meeting_id, &text, Some(&raw_json))?;
        mark_job_succeeded(conn, job_id)?;
        Ok(())
    })();

    if let Err(err) = result {
        fail_job(conn, job_id, &err);
        return Err(err);
    }
    Ok(())
}

/// Persist a terminal failure and log it. Also logs when persisting itself fails,
/// since the job would otherwise stay `running` without a trace.
fn fail_job(conn: &Connection, job_id: &str, err: &AppErrorDto) {
    tracing::warn!(code = %err.code, "job.failed");
    if let Err(mark_err) = mark_job_failed(conn, job_id, &err.code, &err.message) {
        tracing::error!(code = %err.code, mark_code = %mark_err.code, "could not persist job failure");
    }
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// Spawn background work using real HTTP / TOS clients.
pub fn spawn_transcription_job(app: tauri::AppHandle, job_id: String) {
    std::thread::spawn(move || {
        use tauri::Manager;

        let span = tracing::info_span!(
            "transcription",
            job_id = %job_id,
            meeting_id = tracing::field::Empty
        );
        let _entered = span.enter();

        let Some(state) = app.try_state::<crate::AppState>() else {
            tracing::error!("app state unavailable; job left running");
            return;
        };
        let lock_db = || crate::db::lock(&state.db);

        let async_asr = match HttpAsyncClient::new() {
            Ok(client) => client,
            Err(err) => {
                // Previously returned silently, leaving the job `running` forever.
                if let Ok(conn) = lock_db() {
                    fail_job(&conn, &job_id, &err);
                }
                return;
            }
        };
        let tos = HttpTosClient::new();

        let (ctx, tos_config, credentials) = {
            let Ok(conn) = lock_db() else {
                return;
            };
            let loaded = load_work_context(&conn, &job_id).and_then(|ctx| {
                let credentials = credentials::require_credentials()?;
                let tos_config = load_tos_config(&conn)?;
                Ok((ctx, tos_config, credentials))
            });
            match loaded {
                Ok(loaded) => loaded,
                Err(err) => {
                    fail_job(&conn, &job_id, &err);
                    return;
                }
            }
        };
        span.record("meeting_id", tracing::field::display(&ctx.meeting_id));
        let object_key = build_object_key(&ctx.meeting_id, &ctx.source_path);

        // DB lock is not held while uploading / polling.
        let recognize_result = (|| -> CmdResult<(String, String)> {
            if ctx.file_size > meeting_service::ASYNC_MAX_AUDIO_BYTES {
                return Err(AppErrorDto::asr_payload_too_large(
                    meeting_service::ASYNC_MAX_AUDIO_BYTES,
                ));
            }

            tracing::info!(file_size = ctx.file_size, "upload.start");
            let started = Instant::now();
            tos.put_file(&tos_config, &ctx.source_path, &object_key)?;
            tracing::info!(elapsed_ms = elapsed_ms(started), "upload.done");

            let audio_url = tos.pre_sign_get(&tos_config, &object_key, PRESIGN_TTL_SECS)?;
            let format = audio_format_from_path(&ctx.source_path);
            let submit = async_asr.submit(
                &credentials,
                &AsyncSubmitInput {
                    audio_url,
                    format,
                    hotwords: ctx.hotwords.clone(),
                },
            )?;
            tracing::info!(provider_task_id = %submit.request_id, "submit.done");

            set_provider_task_id(&*lock_db()?, &job_id, &submit.request_id)?;

            let started = Instant::now();
            let polled = poll_until_done(
                &async_asr,
                &credentials,
                &submit.request_id,
                submit.log_id.as_deref(),
                ASYNC_POLL_TIMEOUT,
                ASYNC_POLL_INTERVAL,
            );
            tracing::info!(
                elapsed_ms = elapsed_ms(started),
                outcome = if polled.is_ok() {
                    "succeeded"
                } else {
                    "failed"
                },
                "poll.done"
            );
            let output = polled?;

            Ok((output.text, output.raw_json))
        })();

        let Ok(conn) = lock_db() else {
            return;
        };
        let persisted = recognize_result.and_then(|(text, raw_json)| {
            meeting_service::upsert_transcript_from_asr(
                &conn,
                &ctx.meeting_id,
                &text,
                Some(&raw_json),
            )?;
            mark_job_succeeded(&conn, &job_id)
        });
        match persisted {
            Ok(()) => {
                tracing::info!("job.succeeded");
                drop(conn);
                // Best-effort delete — must not fail a successful transcript.
                if let Err(err) = tos.delete_object(&tos_config, &object_key) {
                    tracing::warn!(object_key = %object_key, code = %err.code, "tos.delete failed");
                }
            }
            Err(err) => fail_job(&conn, &job_id, &err),
        }
    });
}

/// Test helper: start + execute with stub recognizers (no threads).
#[cfg(test)]
pub fn run_transcription_with_recognizers(
    conn: &Connection,
    meeting_id: &str,
    async_asr: std::sync::Arc<dyn AsyncRecognizer>,
    tos: std::sync::Arc<dyn ObjectStorage>,
) -> CmdResult<Job> {
    let job = start_transcription_job(conn, meeting_id)?;
    execute_transcription_job(conn, &job.id, async_asr.as_ref(), tos.as_ref())?;
    get_job(conn, &job.id)
}

#[cfg(test)]
pub fn run_transcription_with_poll(
    conn: &Connection,
    meeting_id: &str,
    async_asr: std::sync::Arc<dyn AsyncRecognizer>,
    tos: std::sync::Arc<dyn ObjectStorage>,
    poll_timeout: Duration,
    poll_interval: Duration,
) -> CmdResult<Job> {
    let job = start_transcription_job(conn, meeting_id)?;
    execute_transcription_job_with_poll(
        conn,
        &job.id,
        async_asr.as_ref(),
        tos.as_ref(),
        poll_timeout,
        poll_interval,
    )?;
    get_job(conn, &job.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool::open_memory;
    use crate::models::SettingsUpdate;
    use crate::providers::doubao::{AsyncQueryStatus, AsyncSubmitOutput};
    use crate::services::credentials::{reset_for_test, set_credentials, set_tos_credentials};
    use crate::services::meeting_service::create_from_file;
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// Larger than the removed 20 MiB flash threshold; still goes through the same path.
    const LARGE_AUDIO_BYTES: u64 = 20 * 1024 * 1024 + 1;

    struct StubAsync {
        queries_before_success: usize,
        calls: AtomicUsize,
        always_progress: bool,
        fail: bool,
    }

    impl AsyncRecognizer for StubAsync {
        fn submit(
            &self,
            _credentials: &credentials::DoubaoCredentials,
            input: &AsyncSubmitInput,
        ) -> CmdResult<AsyncSubmitOutput> {
            assert!(!input.audio_url.is_empty());
            let body = crate::providers::doubao::async_client::build_async_submit_body(input);
            assert!(!body.to_string().contains("context_text"));
            Ok(AsyncSubmitOutput {
                request_id: "async-task-1".into(),
                log_id: None,
            })
        }

        fn query(
            &self,
            _credentials: &credentials::DoubaoCredentials,
            _request_id: &str,
            _log_id: Option<&str>,
        ) -> CmdResult<AsyncQueryStatus> {
            if self.fail {
                return Ok(AsyncQueryStatus::Failed {
                    message: "boom".into(),
                });
            }
            if self.always_progress {
                return Ok(AsyncQueryStatus::InProgress);
            }
            let n = self.calls.fetch_add(1, Ordering::SeqCst);
            if n + 1 >= self.queries_before_success {
                Ok(AsyncQueryStatus::Succeeded {
                    text: "seed asr transcript".into(),
                    raw_json: r#"{"result":{"text":"seed asr transcript"}}"#.into(),
                })
            } else {
                Ok(AsyncQueryStatus::InProgress)
            }
        }
    }

    struct StubTos {
        put_calls: AtomicUsize,
        delete_calls: AtomicUsize,
        delete_fails: bool,
    }

    impl ObjectStorage for StubTos {
        fn put_file(
            &self,
            _config: &TosConfig,
            _local_path: &str,
            _object_key: &str,
        ) -> CmdResult<()> {
            self.put_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn pre_sign_get(
            &self,
            _config: &TosConfig,
            object_key: &str,
            _expires_secs: i64,
        ) -> CmdResult<String> {
            Ok(format!("https://tos.example/{object_key}?sig=x"))
        }

        fn delete_object(&self, _config: &TosConfig, _object_key: &str) -> CmdResult<()> {
            self.delete_calls.fetch_add(1, Ordering::SeqCst);
            if self.delete_fails {
                Err(AppErrorDto::tos_upload_error("delete failed"))
            } else {
                Ok(())
            }
        }
    }

    fn temp_audio() -> (std::path::PathBuf, String) {
        let path = std::env::temp_dir().join(format!("meetphant-asr-{}.wav", Uuid::new_v4()));
        let mut f = fs::File::create(&path).expect("create");
        f.write_all(b"fake-audio").expect("write");
        let s = path.to_str().unwrap().to_string();
        (path, s)
    }

    fn large_temp_audio(size: u64) -> (std::path::PathBuf, String) {
        let path = std::env::temp_dir().join(format!("meetphant-large-{}.wav", Uuid::new_v4()));
        {
            let f = fs::File::create(&path).expect("create");
            f.set_len(size).expect("size");
        }
        let s = path.to_str().unwrap().to_string();
        (path, s)
    }

    fn configure_tos(conn: &Connection) {
        set_tos_credentials("ak", "sk").unwrap();
        settings_service::update_settings(
            conn,
            SettingsUpdate {
                tos_region: Some("cn-beijing".into()),
                tos_bucket: Some("meetphant-bucket".into()),
                ..Default::default()
            },
        )
        .unwrap();
    }

    fn async_ok() -> Arc<StubAsync> {
        Arc::new(StubAsync {
            queries_before_success: 1,
            calls: AtomicUsize::new(0),
            always_progress: false,
            fail: false,
        })
    }

    fn tos_ok() -> Arc<StubTos> {
        Arc::new(StubTos {
            put_calls: AtomicUsize::new(0),
            delete_calls: AtomicUsize::new(0),
            delete_fails: false,
        })
    }

    #[test]
    fn small_file_goes_through_tos_and_seed_asr() {
        reset_for_test();
        set_credentials("doubao-key").unwrap();
        let conn = open_memory().unwrap();
        configure_tos(&conn);
        let (path, path_str) = temp_audio();
        let meeting = create_from_file(&conn, &path_str).unwrap();

        let tos = tos_ok();
        let job = run_transcription_with_recognizers(&conn, &meeting.id, async_ok(), tos.clone())
            .unwrap();
        assert_eq!(job.status, JOB_STATUS_SUCCEEDED);
        assert_eq!(tos.put_calls.load(Ordering::SeqCst), 1);
        let transcript = meeting_service::get_transcript(&conn, &meeting.id).unwrap();
        assert_eq!(transcript.text, "seed asr transcript");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn job_transitions_failed_on_provider_error() {
        reset_for_test();
        set_credentials("doubao-key").unwrap();
        let conn = open_memory().unwrap();
        configure_tos(&conn);
        let (path, path_str) = temp_audio();
        let meeting = create_from_file(&conn, &path_str).unwrap();

        let stub = Arc::new(StubAsync {
            queries_before_success: 1,
            calls: AtomicUsize::new(0),
            always_progress: false,
            fail: true,
        });

        let err = run_transcription_with_recognizers(&conn, &meeting.id, stub, tos_ok())
            .expect_err("fail");
        assert_eq!(err.code, "ASR_PROVIDER_ERROR");

        let jobs: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT status, error_code FROM jobs WHERE meeting_id = ?1")
                .unwrap();
            let rows = stmt
                .query_map(rusqlite::params![meeting.id], |row| {
                    let status: String = row.get(0)?;
                    let code: String = row.get(1)?;
                    Ok(format!("{status}:{code}"))
                })
                .unwrap();
            rows.map(|r| r.unwrap()).collect()
        };
        assert_eq!(jobs, vec!["failed:ASR_PROVIDER_ERROR".to_string()]);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn start_without_credentials_errors() {
        reset_for_test();
        let conn = open_memory().unwrap();
        let (path, path_str) = temp_audio();
        let meeting = create_from_file(&conn, &path_str).unwrap();
        let err = start_transcription_job(&conn, &meeting.id).expect_err("no creds");
        assert_eq!(err.code, "ASR_NOT_CONFIGURED");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn start_rejects_over_async_cap() {
        reset_for_test();
        set_credentials("doubao-key").unwrap();
        let conn = open_memory().unwrap();
        let path = std::env::temp_dir().join(format!("meetphant-oversize-{}.wav", Uuid::new_v4()));
        {
            let f = fs::File::create(&path).expect("create");
            f.set_len(meeting_service::ASYNC_MAX_AUDIO_BYTES + 1)
                .expect("size");
        }
        let meeting_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO meetings (id, source_path, title, created_at) VALUES (?1, ?2, NULL, ?3)",
            rusqlite::params![meeting_id, path.to_str().unwrap(), Utc::now().to_rfc3339()],
        )
        .unwrap();
        let err = start_transcription_job(&conn, &meeting_id).expect_err("too big");
        assert_eq!(err.code, "ASR_PAYLOAD_TOO_LARGE");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn small_file_without_tos_errors() {
        reset_for_test();
        set_credentials("doubao-key").unwrap();
        let conn = open_memory().unwrap();
        assert!(!settings_service::is_tos_configured(&conn));
        let (path, path_str) = temp_audio();
        let meeting = create_from_file(&conn, &path_str).unwrap();
        let err = start_transcription_job(&conn, &meeting.id).expect_err("no tos");
        assert_eq!(err.code, "TOS_NOT_CONFIGURED");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn large_file_without_tos_errors() {
        reset_for_test();
        set_credentials("doubao-key").unwrap();
        let conn = open_memory().unwrap();
        let (path, path_str) = large_temp_audio(LARGE_AUDIO_BYTES);
        let meeting = create_from_file(&conn, &path_str).unwrap();
        let err = start_transcription_job(&conn, &meeting.id).expect_err("no tos");
        assert_eq!(err.code, "TOS_NOT_CONFIGURED");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn large_file_succeeds_with_stubs() {
        reset_for_test();
        set_credentials("doubao-key").unwrap();
        let conn = open_memory().unwrap();
        configure_tos(&conn);
        let (path, path_str) = large_temp_audio(LARGE_AUDIO_BYTES);
        let meeting = create_from_file(&conn, &path_str).unwrap();

        let tos = tos_ok();
        let job = run_transcription_with_recognizers(&conn, &meeting.id, async_ok(), tos.clone())
            .unwrap();
        assert_eq!(job.status, JOB_STATUS_SUCCEEDED);
        assert_eq!(tos.put_calls.load(Ordering::SeqCst), 1);
        assert_eq!(tos.delete_calls.load(Ordering::SeqCst), 1);

        let task_id: Option<String> = conn
            .query_row(
                "SELECT provider_task_id FROM jobs WHERE id = ?1",
                rusqlite::params![job.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(task_id.as_deref(), Some("async-task-1"));

        let transcript = meeting_service::get_transcript(&conn, &meeting.id).unwrap();
        assert_eq!(transcript.text, "seed asr transcript");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn async_delete_failure_does_not_fail_job() {
        reset_for_test();
        set_credentials("doubao-key").unwrap();
        let conn = open_memory().unwrap();
        configure_tos(&conn);
        let (path, path_str) = large_temp_audio(LARGE_AUDIO_BYTES);
        let meeting = create_from_file(&conn, &path_str).unwrap();

        let tos = Arc::new(StubTos {
            put_calls: AtomicUsize::new(0),
            delete_calls: AtomicUsize::new(0),
            delete_fails: true,
        });
        let job = run_transcription_with_recognizers(&conn, &meeting.id, async_ok(), tos).unwrap();
        assert_eq!(job.status, JOB_STATUS_SUCCEEDED);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn async_poll_timeout_marks_failed() {
        reset_for_test();
        set_credentials("doubao-key").unwrap();
        let conn = open_memory().unwrap();
        configure_tos(&conn);
        let (path, path_str) = large_temp_audio(LARGE_AUDIO_BYTES);
        let meeting = create_from_file(&conn, &path_str).unwrap();

        let async_asr = Arc::new(StubAsync {
            queries_before_success: 1000,
            calls: AtomicUsize::new(0),
            always_progress: true,
            fail: false,
        });

        let err = run_transcription_with_poll(
            &conn,
            &meeting.id,
            async_asr,
            tos_ok(),
            Duration::from_millis(30),
            Duration::from_millis(5),
        )
        .expect_err("timeout");
        assert_eq!(err.code, "ASR_TIMEOUT");

        let (status, error_code, error_message): (String, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT status, error_code, error_message FROM jobs WHERE meeting_id = ?1",
                rusqlite::params![meeting.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(status, JOB_STATUS_FAILED);
        assert_eq!(error_code.as_deref(), Some("ASR_TIMEOUT"));
        assert!(
            error_message.as_ref().is_some_and(|m| !m.trim().is_empty()),
            "failed job must persist a non-empty error_message"
        );
        let _ = fs::remove_file(path);
    }
}
