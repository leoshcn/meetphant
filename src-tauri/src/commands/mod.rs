pub mod health;
pub mod jobs;
pub mod meetings;
pub mod recording;
pub mod settings;
pub mod summary;
pub mod tray;

use crate::error::{AppErrorDto, CmdResult};

/// Run blocking command work (HTTP, FFmpeg, audio devices, file IO) on Tauri's
/// blocking pool so it never runs on the main thread or a tokio worker.
/// `reqwest::blocking` must not run inside an async context.
pub(crate) async fn run_blocking<T, F>(f: F) -> CmdResult<T>
where
    F: FnOnce() -> CmdResult<T> + Send + 'static,
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|_| AppErrorDto::internal("Background task failed"))?
}
