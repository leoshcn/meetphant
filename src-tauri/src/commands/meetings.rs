use std::collections::BTreeMap;

use tauri::State;

use crate::error::CmdResult;
use crate::models::{Meeting, Transcript};
use crate::services::meeting_service;
use crate::AppState;

/// `rename_all = "snake_case"` keeps IPC args aligned with api-shape.md / `src/ipc`.
#[tauri::command(rename_all = "snake_case")]
pub fn meetings_create(state: State<'_, AppState>) -> CmdResult<Meeting> {
    let conn = state
        .db
        .lock()
        .map_err(|_| crate::error::AppErrorDto::internal("Database lock poisoned"))?;
    meeting_service::create_draft(&conn)
}

#[tauri::command(rename_all = "snake_case")]
pub fn meetings_create_from_file(state: State<'_, AppState>, path: String) -> CmdResult<Meeting> {
    let conn = state
        .db
        .lock()
        .map_err(|_| crate::error::AppErrorDto::internal("Database lock poisoned"))?;
    meeting_service::create_from_file(&conn, &path)
}

#[tauri::command(rename_all = "snake_case")]
pub fn meetings_attach_source(
    state: State<'_, AppState>,
    meeting_id: String,
    path: String,
) -> CmdResult<Meeting> {
    let conn = state
        .db
        .lock()
        .map_err(|_| crate::error::AppErrorDto::internal("Database lock poisoned"))?;
    meeting_service::attach_source(&conn, &meeting_id, &path)
}

#[tauri::command(rename_all = "snake_case")]
pub fn meetings_list(state: State<'_, AppState>) -> CmdResult<Vec<Meeting>> {
    let conn = state
        .db
        .lock()
        .map_err(|_| crate::error::AppErrorDto::internal("Database lock poisoned"))?;
    meeting_service::list_meetings(&conn)
}

#[tauri::command(rename_all = "snake_case")]
pub fn meetings_get(state: State<'_, AppState>, meeting_id: String) -> CmdResult<Meeting> {
    let conn = state
        .db
        .lock()
        .map_err(|_| crate::error::AppErrorDto::internal("Database lock poisoned"))?;
    meeting_service::get_meeting(&conn, &meeting_id)
}

#[tauri::command(rename_all = "snake_case")]
pub fn meetings_rename(
    state: State<'_, AppState>,
    meeting_id: String,
    title: String,
) -> CmdResult<Meeting> {
    let conn = state
        .db
        .lock()
        .map_err(|_| crate::error::AppErrorDto::internal("Database lock poisoned"))?;
    meeting_service::rename_meeting(&conn, &meeting_id, &title)
}

#[tauri::command(rename_all = "snake_case")]
pub fn meetings_delete(state: State<'_, AppState>, meeting_id: String) -> CmdResult<()> {
    let conn = state
        .db
        .lock()
        .map_err(|_| crate::error::AppErrorDto::internal("Database lock poisoned"))?;
    meeting_service::delete_meeting(&conn, &meeting_id)
}

#[tauri::command(rename_all = "snake_case")]
pub fn meetings_get_transcript(
    state: State<'_, AppState>,
    meeting_id: String,
) -> CmdResult<Transcript> {
    let conn = state
        .db
        .lock()
        .map_err(|_| crate::error::AppErrorDto::internal("Database lock poisoned"))?;
    meeting_service::get_transcript(&conn, &meeting_id)
}

#[tauri::command(rename_all = "snake_case")]
pub fn meetings_update_speakers(
    state: State<'_, AppState>,
    meeting_id: String,
    speaker_names: BTreeMap<String, String>,
) -> CmdResult<Transcript> {
    let conn = state
        .db
        .lock()
        .map_err(|_| crate::error::AppErrorDto::internal("Database lock poisoned"))?;
    meeting_service::update_speakers(&conn, &meeting_id, speaker_names)
}
