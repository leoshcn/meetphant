use tauri::{AppHandle, Manager, State};

use super::run_blocking;
use crate::error::{CmdResult, CmdResultExt};
use crate::models::{Settings, SettingsUpdate};
use crate::services::{self, SettingsTestResult};
use crate::AppState;

#[tauri::command(rename_all = "snake_case")]
pub fn settings_get(state: State<'_, AppState>) -> CmdResult<Settings> {
    crate::db::with(&state.db, services::get_settings).log_cmd("settings_get")
}

#[tauri::command(rename_all = "snake_case")]
pub fn settings_update(state: State<'_, AppState>, update: SettingsUpdate) -> CmdResult<Settings> {
    crate::db::with(&state.db, |conn| services::update_settings(conn, update))
        .log_cmd("settings_update")
}

#[tauri::command(rename_all = "snake_case")]
pub fn settings_clear_doubao_credentials(state: State<'_, AppState>) -> CmdResult<Settings> {
    crate::db::with(&state.db, services::clear_doubao_credentials)
        .log_cmd("settings_clear_doubao_credentials")
}

#[tauri::command(rename_all = "snake_case")]
pub fn settings_clear_summary_llm_credentials(state: State<'_, AppState>) -> CmdResult<Settings> {
    crate::db::with(&state.db, services::clear_summary_llm_credentials)
        .log_cmd("settings_clear_summary_llm_credentials")
}

#[tauri::command(rename_all = "snake_case")]
pub fn settings_clear_tos_credentials(state: State<'_, AppState>) -> CmdResult<Settings> {
    crate::db::with(&state.db, services::clear_tos_credentials)
        .log_cmd("settings_clear_tos_credentials")
}

/// Probe Doubao credentials. Optional overrides merge with keyring; never persists.
#[tauri::command(rename_all = "snake_case")]
pub async fn settings_test_doubao(doubao_api_key: Option<String>) -> CmdResult<SettingsTestResult> {
    run_blocking(move || services::test_doubao(doubao_api_key.as_deref()))
        .await
        .log_cmd("settings_test_doubao")
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
    .log_cmd("settings_test_tos")
}

/// Probe the summary LLM with a minimal chat completion using the configured model.
/// Optional overrides merge with keyring/SQLite; never persists.
#[tauri::command(rename_all = "snake_case")]
pub async fn settings_test_summary_llm(
    app: AppHandle,
    api_key: Option<String>,
    provider: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
) -> CmdResult<SettingsTestResult> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        services::test_summary_llm(
            &state.db,
            services::SummaryLlmTestOverrides {
                api_key: api_key.as_deref(),
                provider: provider.as_deref(),
                base_url: base_url.as_deref(),
                model: model.as_deref(),
            },
        )
    })
    .await
    .log_cmd("settings_test_summary_llm")
}
