use tauri::{AppHandle, Manager, State};

use super::run_blocking;
use crate::error::{CmdResult, CmdResultExt};
use crate::models::Job;
use crate::services::transcription_service;
use crate::AppState;

/// `rename_all = "snake_case"` keeps IPC args aligned with api-shape.md / `src/ipc`.
#[tauri::command(rename_all = "snake_case")]
pub async fn jobs_start_transcription(app: AppHandle, meeting_id: String) -> CmdResult<Job> {
    run_blocking(move || {
        let job = {
            let state = app.state::<AppState>();
            let conn = crate::db::lock(&state.db)?;
            transcription_service::start_transcription_job(&conn, &meeting_id)?
        };
        transcription_service::spawn_transcription_job(app, job.id.clone());
        Ok(job)
    })
    .await
    .log_cmd("jobs_start_transcription")
}

#[tauri::command(rename_all = "snake_case")]
pub fn jobs_get(state: State<'_, AppState>, job_id: String) -> CmdResult<Job> {
    crate::db::with(&state.db, |conn| {
        transcription_service::get_job(conn, &job_id)
    })
    .log_cmd("jobs_get")
}
