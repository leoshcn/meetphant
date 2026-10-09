use tauri::AppHandle;

use crate::error::{AppErrorDto, CmdResult};
use crate::services::tray_service;

/// Hide the main window and show the recording tray icon.
#[tauri::command]
pub fn recording_hide_to_tray(app: AppHandle) -> CmdResult<()> {
    tray_service::set_recording_tray_visible(&app, true).map_err(AppErrorDto::internal)?;
    tray_service::hide_main_window(&app).map_err(AppErrorDto::internal)?;
    Ok(())
}

/// Show the main window and hide the recording tray icon.
#[tauri::command]
pub fn recording_restore_from_tray(app: AppHandle) -> CmdResult<()> {
    tray_service::show_main_window(&app).map_err(AppErrorDto::internal)?;
    tray_service::set_recording_tray_visible(&app, false).map_err(AppErrorDto::internal)?;
    Ok(())
}

/// Hide the tray without changing main-window visibility (e.g. after stop while main is already shown).
#[tauri::command]
pub fn recording_hide_tray(app: AppHandle) -> CmdResult<()> {
    tray_service::set_recording_tray_visible(&app, false).map_err(AppErrorDto::internal)?;
    Ok(())
}
