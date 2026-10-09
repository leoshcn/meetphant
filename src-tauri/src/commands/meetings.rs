use std::collections::BTreeMap;

use tauri::{AppHandle, Manager, State};

use super::run_blocking;
use crate::error::{CmdResult, CmdResultExt};
use crate::models::{Meeting, Transcript};
use crate::services::meeting_service;
use crate::AppState;

/// `rename_all = "snake_case"` keeps IPC args aligned with api-shape.md / `src/ipc`.
#[tauri::command(rename_all = "snake_case")]
pub fn meetings_create(state: State<'_, AppState>) -> CmdResult<Meeting> {
    crate::db::with(&state.db, meeting_service::create_draft).log_cmd("meetings_create")
}

#[tauri::command(rename_all = "snake_case")]
pub async fn meetings_create_from_file(app: AppHandle, path: String) -> CmdResult<Meeting> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        let conn = crate::db::lock(&state.db)?;
        meeting_service::create_from_file(&conn, &path)
    })
    .await
    .log_cmd("meetings_create_from_file")
}

#[tauri::command(rename_all = "snake_case")]
pub async fn meetings_attach_source(
    app: AppHandle,
    meeting_id: String,
    path: String,
) -> CmdResult<Meeting> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        let conn = crate::db::lock(&state.db)?;
        meeting_service::attach_source(&conn, &meeting_id, &path)
    })
    .await
    .log_cmd("meetings_attach_source")
}

#[tauri::command(rename_all = "snake_case")]
pub fn meetings_list(state: State<'_, AppState>) -> CmdResult<Vec<Meeting>> {
    crate::db::with(&state.db, meeting_service::list_meetings).log_cmd("meetings_list")
}

#[tauri::command(rename_all = "snake_case")]
pub fn meetings_get(state: State<'_, AppState>, meeting_id: String) -> CmdResult<Meeting> {
    crate::db::with(&state.db, |conn| {
        meeting_service::get_meeting(conn, &meeting_id)
    })
    .log_cmd("meetings_get")
}

#[tauri::command(rename_all = "snake_case")]
pub fn meetings_rename(
    state: State<'_, AppState>,
    meeting_id: String,
    title: String,
) -> CmdResult<Meeting> {
    crate::db::with(&state.db, |conn| {
        meeting_service::rename_meeting(conn, &meeting_id, &title)
    })
    .log_cmd("meetings_rename")
}

#[tauri::command(rename_all = "snake_case")]
pub fn meetings_delete(state: State<'_, AppState>, meeting_id: String) -> CmdResult<()> {
    crate::db::with(&state.db, |conn| {
        meeting_service::delete_meeting(conn, &meeting_id)
    })
    .log_cmd("meetings_delete")
}

#[tauri::command(rename_all = "snake_case")]
pub fn meetings_get_transcript(
    state: State<'_, AppState>,
    meeting_id: String,
) -> CmdResult<Transcript> {
    crate::db::with(&state.db, |conn| {
        meeting_service::get_transcript(conn, &meeting_id)
    })
    .log_cmd("meetings_get_transcript")
}

#[tauri::command(rename_all = "snake_case")]
pub fn meetings_update_speakers(
    state: State<'_, AppState>,
    meeting_id: String,
    speaker_names: BTreeMap<String, String>,
) -> CmdResult<Transcript> {
    crate::db::with(&state.db, |conn| {
        meeting_service::update_speakers(conn, &meeting_id, speaker_names)
    })
    .log_cmd("meetings_update_speakers")
}
