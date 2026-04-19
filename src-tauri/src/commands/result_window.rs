use tauri::{AppHandle, Manager};
use crate::vlm::state::{self, VlmSnapshot};

#[tauri::command]
pub fn get_latest_vlm_state() -> Option<VlmSnapshot> {
    state::snapshot()
}

#[tauri::command]
pub fn show_result_window(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("result")
        .ok_or_else(|| "result window not found".to_string())?;

    window.center().map_err(|err| err.to_string())?;
    window.show().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn hide_result_window(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("result")
        .ok_or_else(|| "result window not found".to_string())?;

    window.hide().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn show_settings_window(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("settings")
        .ok_or_else(|| "settings window not found".to_string())?;

    window.center().map_err(|err| err.to_string())?;
    window.show().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn hide_settings_window(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("settings")
        .ok_or_else(|| "settings window not found".to_string())?;

    window.hide().map_err(|err| err.to_string())
}
