use tauri::{AppHandle, Manager};

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
