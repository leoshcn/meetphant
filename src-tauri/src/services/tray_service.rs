//! Recording-session tray: visible only while recording; restores main or exits.

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

pub const TRAY_ID: &str = "recording-tray";

fn show_main_and_focus(app: &AppHandle) {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.unminimize();
        let _ = main.show();
        let _ = main.set_focus();
        let _ = main.emit("recording:focus-request", ());
    }
}

fn is_recording(app: &AppHandle) -> bool {
    app.try_state::<crate::AppState>()
        .and_then(|state| state.recording.status().ok())
        .is_some_and(|status| status.state == "recording")
}

fn on_quit_requested(app: &AppHandle) {
    if is_recording(app) {
        // Dialog lives in the main webview — show it first.
        show_main_and_focus(app);
        if let Some(main) = app.get_webview_window("main") {
            let _ = main.emit("recording:close-requested", ());
        }
    } else {
        app.exit(0);
    }
}

/// Build a hidden tray icon; shown only while a recording session is active.
pub fn setup_recording_tray(app: &AppHandle) -> Result<(), String> {
    let open = MenuItem::with_id(app, "open", "打开 Meetphant", true, None::<&str>)
        .map_err(|e| format!("Failed to create tray open item: {e}"))?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)
        .map_err(|e| format!("Failed to create tray quit item: {e}"))?;
    let menu = Menu::with_items(app, &[&open, &quit])
        .map_err(|e| format!("Failed to create tray menu: {e}"))?;

    let icon = app
        .default_window_icon()
        .ok_or_else(|| "Missing default window icon for tray".to_string())?
        .clone();

    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .menu(&menu)
        .tooltip("Meetphant")
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main_and_focus(app),
            "quit" => on_quit_requested(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_and_focus(tray.app_handle());
            }
        })
        .build(app)
        .map_err(|e| format!("Failed to build tray icon: {e}"))?;

    tray.set_visible(false)
        .map_err(|e| format!("Failed to hide tray icon: {e}"))?;
    Ok(())
}

pub fn set_recording_tray_visible(app: &AppHandle, visible: bool) -> Result<(), String> {
    let tray = app
        .tray_by_id(TRAY_ID)
        .ok_or_else(|| "Recording tray is not available".to_string())?;
    if visible {
        let _ = tray.set_tooltip(Some("Meetphant · 正在录音"));
    } else {
        let _ = tray.set_tooltip(Some("Meetphant"));
    }
    tray.set_visible(visible)
        .map_err(|e| format!("Failed to set tray visibility: {e}"))?;
    Ok(())
}

pub fn hide_main_window(app: &AppHandle) -> Result<(), String> {
    let main = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;
    main.hide()
        .map_err(|e| format!("Failed to hide main window: {e}"))?;
    Ok(())
}

pub fn show_main_window(app: &AppHandle) -> Result<(), String> {
    show_main_and_focus(app);
    Ok(())
}
