use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager};
use crate::window_state;
use crate::vlm::state::{self, VlmSnapshot};

#[tauri::command]
pub fn get_latest_vlm_state() -> Option<VlmSnapshot> {
    eprintln!("[cmd] get_latest_vlm_state called");
    state::snapshot()
}

#[tauri::command]
pub fn get_window_state() -> crate::window_state::WindowState {
    crate::window_state::get()
}

#[tauri::command]
pub fn show_result_window(app: AppHandle) -> Result<(), String> {
    let state = window_state::get();
    let window = app
        .get_webview_window("result")
        .ok_or_else(|| "result window not found".to_string())?;

    window
        .set_size(LogicalSize::new(
            f64::from(state.popup_width),
            f64::from(state.popup_height),
        ))
        .map_err(|err| err.to_string())?;
    if let (Some(x), Some(y)) = (state.popup_x, state.popup_y) {
        window
            .set_position(LogicalPosition::new(f64::from(x), f64::from(y)))
            .map_err(|err| err.to_string())?;
    } else {
        window.center().map_err(|err| err.to_string())?;
    }
    window
        .set_always_on_top(state.popup_topmost)
        .map_err(|err| err.to_string())?;
    window.show().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn set_popup_topmost(app: AppHandle, value: bool) -> Result<(), String> {
    let window = app
        .get_webview_window("result")
        .ok_or_else(|| "result window not found".to_string())?;

    window_state::set_popup_topmost(value);
    window
        .set_always_on_top(value)
        .map_err(|err| err.to_string())?;
    window.show().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn save_popup_window_geometry(x: i32, y: i32, w: u32, h: u32) -> Result<(), String> {
    window_state::set_popup_position(x, y);
    window_state::set_popup_size(w, h);
    Ok(())
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
