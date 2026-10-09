use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

use super::run_blocking;
use crate::error::{AppErrorDto, CmdResult, CmdResultExt};

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

#[tauri::command]
pub fn app_health() -> CmdResult<HealthResponse> {
    Ok(HealthResponse {
        status: "ok".into(),
        version: env!("CARGO_PKG_VERSION").into(),
    })
}

/// Open the log directory in the system file manager (Settings → About).
#[tauri::command]
pub async fn logs_open_dir(app: AppHandle) -> CmdResult<()> {
    run_blocking(move || {
        let dir = app
            .path()
            .app_log_dir()
            .map_err(|_| AppErrorDto::io_error("Could not resolve the log directory"))?;
        std::fs::create_dir_all(&dir)
            .map_err(|_| AppErrorDto::io_error("Could not create the log directory"))?;
        app.opener()
            .open_path(dir.to_string_lossy(), None::<&str>)
            .map_err(|_| AppErrorDto::io_error("Could not open the log directory"))
    })
    .await
    .log_cmd("logs_open_dir")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_health_ok() {
        let res = app_health().expect("health");
        assert_eq!(res.status, "ok");
        assert!(!res.version.is_empty());
    }
}
