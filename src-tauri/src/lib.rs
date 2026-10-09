mod commands;
mod db;
mod error;
mod logging;
mod models;
mod providers;
mod services;

use std::sync::Mutex;

use rusqlite::Connection;
use tauri::{Emitter, Manager};

pub use error::{AppErrorDto, CmdResult};
use services::recording_service::RecordingSession;

pub struct AppState {
    pub db: Mutex<Connection>,
    pub recording: RecordingSession,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // First, so migrations and DB open failures below are captured.
            if let Ok(log_dir) = app.path().app_log_dir() {
                logging::init(&log_dir);
            }

            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("Failed to resolve app data dir: {e}"))?;
            std::fs::create_dir_all(&data_dir)
                .map_err(|e| format!("Failed to create app data dir: {e}"))?;

            // One-time migration from the pre-rename `Meetly` app identity;
            // no-op on fresh installs or once already migrated.
            let db_path = data_dir.join("meetphant.db");
            services::migration::migrate_app_data_dir(&data_dir, &db_path);
            services::migration::migrate_recordings_dir();
            services::credentials::migrate_legacy_credentials();

            let ffmpeg_dir = data_dir.join("ffmpeg");
            std::fs::create_dir_all(&ffmpeg_dir)
                .map_err(|e| format!("Failed to create FFmpeg dir: {e}"))?;
            services::ffmpeg_service::init_install_dir(ffmpeg_dir);
            let conn = db::open_connection(&db_path)
                .map_err(|e| format!("Failed to open database: {}", e.message))?;
            app.manage(AppState {
                db: Mutex::new(conn),
                recording: RecordingSession::spawn(),
            });
            services::tray_service::setup_recording_tray(app.handle())
                .map_err(|e| format!("Failed to set up recording tray: {e}"))?;
            Ok(())
        })
        .on_window_event(|window, event| {
            let tauri::WindowEvent::CloseRequested { api, .. } = event else {
                return;
            };

            match window.label() {
                "recorder-widget" => {
                    api.prevent_close();
                }
                "main" => {
                    let recording = window
                        .try_state::<AppState>()
                        .and_then(|state| state.recording.status().ok())
                        .is_some_and(|status| status.state == "recording");

                    if recording {
                        api.prevent_close();
                        let _ = window.emit("recording:close-requested", ());
                    } else {
                        // Persistent recorder-widget would otherwise keep the process alive.
                        window.app_handle().exit(0);
                    }
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::health::app_health,
            commands::health::logs_open_dir,
            commands::settings::settings_get,
            commands::settings::settings_update,
            commands::settings::settings_clear_doubao_credentials,
            commands::settings::settings_clear_dashscope_credentials,
            commands::settings::settings_clear_tos_credentials,
            commands::settings::settings_test_doubao,
            commands::settings::settings_test_tos,
            commands::settings::settings_test_dashscope,
            commands::meetings::meetings_create,
            commands::meetings::meetings_create_from_file,
            commands::meetings::meetings_attach_source,
            commands::meetings::meetings_list,
            commands::meetings::meetings_get,
            commands::meetings::meetings_rename,
            commands::meetings::meetings_delete,
            commands::meetings::meetings_get_transcript,
            commands::meetings::meetings_update_speakers,
            commands::jobs::jobs_start_transcription,
            commands::jobs::jobs_get,
            commands::summary::summary_generate,
            commands::summary::summary_get,
            commands::recording::record_list_input_devices,
            commands::recording::record_start,
            commands::recording::record_stop,
            commands::recording::record_status,
            commands::recording::ffmpeg_status,
            commands::recording::ffmpeg_download,
            commands::tray::recording_hide_to_tray,
            commands::tray::recording_restore_from_tray,
            commands::tray::recording_hide_tray,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
