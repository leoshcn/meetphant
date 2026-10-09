use tauri::{AppHandle, Manager, State};

use super::run_blocking;
use crate::error::{CmdResult, CmdResultExt};
use crate::services::ffmpeg_service::{self, FfmpegStatus};
use crate::services::recording_service::{
    self, DevicesResponse, RecordStartResponse, RecordStatusResponse, RecordStopResponse,
};
use crate::AppState;

#[tauri::command(rename_all = "snake_case")]
pub async fn record_list_input_devices() -> CmdResult<DevicesResponse> {
    run_blocking(recording_service::list_input_devices)
        .await
        .log_cmd("record_list_input_devices")
}

#[tauri::command(rename_all = "snake_case")]
pub async fn record_start(
    app: AppHandle,
    device_id: Option<String>,
) -> CmdResult<RecordStartResponse> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        let recording_dir = {
            let conn = crate::db::lock(&state.db)?;
            crate::services::get_settings(&conn)?.recording_dir
        };
        state.recording.start(&recording_dir, device_id.as_deref())
    })
    .await
    .log_cmd("record_start")
}

/// Waits for the WAV → M4A encode, so it must stay off the main thread.
#[tauri::command(rename_all = "snake_case")]
pub async fn record_stop(app: AppHandle) -> CmdResult<RecordStopResponse> {
    run_blocking(move || app.state::<AppState>().recording.stop())
        .await
        .log_cmd("record_stop")
}

#[tauri::command(rename_all = "snake_case")]
pub fn record_status(state: State<'_, AppState>) -> CmdResult<RecordStatusResponse> {
    state.recording.status().log_cmd("record_status")
}

#[tauri::command(rename_all = "snake_case")]
pub fn ffmpeg_status() -> CmdResult<FfmpegStatus> {
    Ok(ffmpeg_service::status())
}

#[tauri::command(rename_all = "snake_case")]
pub fn ffmpeg_download(app: AppHandle) -> CmdResult<FfmpegStatus> {
    ffmpeg_service::start_download(Some(app)).log_cmd("ffmpeg_download")
}
