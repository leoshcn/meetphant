use tauri::{AppHandle, Manager, State};

use super::run_blocking;
use crate::error::{CmdResult, CmdResultExt};
use crate::models::Summary;
use crate::services;
use crate::AppState;

/// `rename_all = "snake_case"` keeps IPC args aligned with api-shape.md / `src/ipc`.
#[tauri::command(rename_all = "snake_case")]
pub async fn summary_generate(
    app: AppHandle,
    meeting_id: String,
    language: String,
) -> CmdResult<Summary> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        services::generate_summary_http(&state.db, &meeting_id, &language)
    })
    .await
    .log_cmd("summary_generate")
}

#[tauri::command(rename_all = "snake_case")]
pub fn summary_get(state: State<'_, AppState>, meeting_id: String) -> CmdResult<Summary> {
    crate::db::with(&state.db, |conn| services::get_summary(conn, &meeting_id))
        .log_cmd("summary_get")
}
