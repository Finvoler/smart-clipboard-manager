use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Manager};

use crate::AppState;

pub fn start_system_integrations(_app: AppHandle, _state: &AppState) {}

pub fn set_run_at_startup(_app: &AppHandle, _enable: bool) -> Result<(), String> {
    Ok(())
}

pub fn set_hide_console_window(_enable: bool) -> Result<(), String> {
    Ok(())
}

pub fn hide_main_window(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.hide().map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn show_main_window(
    app: &AppHandle,
    _last_foreground_window: &Arc<Mutex<Option<isize>>>,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.show().map_err(|error| error.to_string())?;
        window.set_focus().map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn paste_text_and_hide(
    app: &AppHandle,
    text: &str,
    _last_foreground_window: &Arc<Mutex<Option<isize>>>,
) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|error| error.to_string())?;
    clipboard
        .set_text(text.to_string())
        .map_err(|error| error.to_string())?;
    hide_main_window(app)
}
