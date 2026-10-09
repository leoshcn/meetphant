use tauri::{AppHandle, Manager, State};

use super::run_blocking;
use crate::error::CmdResult;
use crate::models::{Settings, SettingsUpdate};
use crate::services::{self, SettingsTestResult};
use crate::AppState;

#[tauri::command(rename_all = "snake_case")]
pub fn settings_get(state: State<'_, AppState>) -> CmdResult<Settings> {
    let conn = state
        .db
        .lock()
        .map_err(|_| crate::error::AppErrorDto::internal("Database lock poisoned"))?;
    services::get_settings(&conn)
}

#[tauri::command(rename_all = "snake_case")]
pub fn settings_update(state: State<'_, AppState>, update: SettingsUpdate) -> CmdResult<Settings> {
    let conn = state
        .db
        .lock()
        .map_err(|_| crate::error::AppErrorDto::internal("Database lock poisoned"))?;
    services::update_settings(&conn, update)
}

#[tauri::command(rename_all = "snake_case")]
pub fn settings_clear_doubao_credentials(state: State<'_, AppState>) -> CmdResult<Settings> {
    let conn = state
        .db
        .lock()
        .map_err(|_| crate::error::AppErrorDto::internal("Database lock poisoned"))?;
    services::clear_doubao_credentials(&conn)
}

#[tauri::command(rename_all = "snake_case")]
pub fn settings_clear_dashscope_credentials(state: State<'_, AppState>) -> CmdResult<Settings> {
    let conn = state
        .db
        .lock()
        .map_err(|_| crate::error::AppErrorDto::internal("Database lock poisoned"))?;
    services::clear_dashscope_credentials(&conn)
}

#[tauri::command(rename_all = "snake_case")]
pub fn settings_clear_tos_credentials(state: State<'_, AppState>) -> CmdResult<Settings> {
    let conn = state
        .db
        .lock()
        .map_err(|_| crate::error::AppErrorDto::internal("Database lock poisoned"))?;
    services::clear_tos_credentials(&conn)
}

/// Probe Doubao credentials. Optional overrides merge with keyring; never persists.
#[tauri::command(rename_all = "snake_case")]
pub async fn settings_test_doubao(doubao_api_key: Option<String>) -> CmdResult<SettingsTestResult> {
    run_blocking(move || services::test_doubao(doubao_api_key.as_deref())).await
}

/// Probe TOS via HeadBucket. Optional overrides merge with keyring/SQLite; never persists.
#[tauri::command(rename_all = "snake_case")]
pub async fn settings_test_tos(
    app: AppHandle,
    tos_access_key_id: Option<String>,
    tos_secret_access_key: Option<String>,
    tos_region: Option<String>,
    tos_bucket: Option<String>,
    tos_endpoint: Option<String>,
) -> CmdResult<SettingsTestResult> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        services::test_tos(
            &state.db,
            tos_access_key_id.as_deref(),
            tos_secret_access_key.as_deref(),
            tos_region.as_deref(),
            tos_bucket.as_deref(),
            tos_endpoint.as_deref(),
        )
    })
    .await
}

/// Probe DashScope via GET /models. Optional override merges with keyring; never persists.
#[tauri::command(rename_all = "snake_case")]
pub async fn settings_test_dashscope(
    dashscope_api_key: Option<String>,
) -> CmdResult<SettingsTestResult> {
    run_blocking(move || services::test_dashscope(dashscope_api_key.as_deref())).await
}
