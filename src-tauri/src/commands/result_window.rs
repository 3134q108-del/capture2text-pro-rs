use std::path::Path;
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, WebviewWindow, WebviewWindowBuilder,
};
use tauri_plugin_opener::OpenerExt;
use crate::window_state::{self, PopupFont};
use crate::vlm::state::{self, VlmSnapshot};

fn is_valid_separator(value: &str) -> bool {
    matches!(
        value,
        "Space" | "Tab" | "LineBreak" | "Comma" | "Semicolon" | "Pipe"
    )
}

pub fn attach_close_handler(window: WebviewWindow) {
    let label = window.label().to_string();
    let window_cloned = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            eprintln!(
                "[window] CloseRequested label={} -> prevent_close + hide",
                label
            );
            api.prevent_close();
            let _ = window_cloned.hide();
        }
    });
}

pub fn ensure_webview_window(app: AppHandle, label: &str) -> Result<WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(label) {
        return Ok(window);
    }

    eprintln!(
        "[window] ensure_webview_window label={} missing, rebuilding",
        label
    );

    let window_config = app
        .config()
        .app
        .windows
        .iter()
        .find(|config| config.label == label)
        .ok_or_else(|| format!("window config not found for label={label}"))?;

    let window = WebviewWindowBuilder::from_config(&app, window_config)
        .map_err(|err| err.to_string())?
        .build()
        .map_err(|err| err.to_string())?;

    attach_close_handler(window.clone());
    Ok(window)
}

#[tauri::command]
pub fn get_latest_vlm_state() -> Option<VlmSnapshot> {
    state::snapshot()
}

#[tauri::command]
pub fn get_window_state() -> crate::window_state::WindowState {
    crate::window_state::get()
}

#[tauri::command]
pub fn show_result_window(app: AppHandle) -> Result<(), String> {
    let state = window_state::get();
    let window = ensure_webview_window(app, "result")?;

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
    let _ = window.unminimize();
    window.show().map_err(|err| err.to_string())?;
    window.set_focus().map_err(|err| err.to_string())
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
pub fn set_popup_font(family: String, size_pt: u32) -> Result<(), String> {
    window_state::set_popup_font(Some(PopupFont { family, size_pt }));
    Ok(())
}

#[tauri::command]
pub fn clear_popup_font() -> Result<(), String> {
    window_state::set_popup_font(None);
    Ok(())
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
    let window = ensure_webview_window(app, "settings")?;

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

#[tauri::command]
pub fn set_save_to_clipboard(value: bool) -> Result<(), String> {
    window_state::set_save_to_clipboard(value);
    Ok(())
}

#[tauri::command]
pub fn set_clipboard_mode(value: String) -> Result<(), String> {
    match window_state::ClipboardMode::from_str(&value) {
        Some(mode) => {
            window_state::set_clipboard_mode(mode);
            Ok(())
        }
        None => Err("invalid clipboard mode".to_string()),
    }
}

#[tauri::command]
pub fn set_popup_show_enabled(value: bool) -> Result<(), String> {
    window_state::set_popup_show_enabled(value);
    Ok(())
}

#[tauri::command]
pub fn set_translate_append_to_clipboard(value: bool) -> Result<(), String> {
    window_state::set_translate_append_to_clipboard(value);
    Ok(())
}

#[tauri::command]
pub fn set_translate_separator(value: String) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("translate separator cannot be empty".to_string());
    }
    if !is_valid_separator(trimmed) {
        return Err(format!("invalid separator: {}", trimmed));
    }
    window_state::set_translate_separator(trimmed.to_string());
    Ok(())
}

#[tauri::command]
pub fn set_log_enabled(value: bool) -> Result<(), String> {
    window_state::set_log_enabled(value);
    Ok(())
}

#[tauri::command]
pub fn set_log_file_path(value: String) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err("log file path cannot be empty".to_string());
    }
    window_state::set_log_file_path(value);
    Ok(())
}

#[tauri::command]
pub fn set_speech_enabled(value: bool) -> Result<(), String> {
    window_state::set_speech_enabled(value);
    Ok(())
}

#[tauri::command]
pub fn write_popup_clipboard() -> Result<(), String> {
    if let Some(snapshot) = crate::vlm::state::snapshot() {
        crate::clipboard::write_capture(&snapshot.original, &snapshot.translated);
    }
    Ok(())
}

#[tauri::command]
pub fn check_llm_health() -> String {
    match crate::vlm::check_health() {
        crate::vlm::HealthStatus::Healthy => "healthy".to_string(),
        crate::vlm::HealthStatus::VlmRuntimeDown => "vlm_runtime_down".to_string(),
        crate::vlm::HealthStatus::ModelMissing { model } => format!("model_missing:{model}"),
        crate::vlm::HealthStatus::Unknown(msg) => format!("unknown:{msg}"),
    }
}

#[tauri::command]
pub fn check_vlm_health() -> String {
    check_llm_health()
}

#[tauri::command]
pub fn open_external_url(app: AppHandle, url: String) -> Result<(), String> {
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn export_settings(target_dir: String) -> Result<String, String> {
    let src = crate::app_paths::data_dir();
    if !src.exists() {
        return Err("settings directory does not exist".to_string());
    }

    let dst = Path::new(&target_dir).join("Capture2TextPro-backup");
    std::fs::create_dir_all(&dst).map_err(|e| e.to_string())?;

    let mut count = 0;
    for entry in std::fs::read_dir(&src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_file() {
            std::fs::copy(&from, &to).map_err(|e| e.to_string())?;
            count += 1;
        }
    }

    Ok(format!("exported {count} files to {}", dst.display()))
}

#[tauri::command]
pub fn import_settings(source_dir: String) -> Result<String, String> {
    let src = Path::new(&source_dir);
    if !src.exists() || !src.is_dir() {
        return Err("source directory does not exist".to_string());
    }

    let dst = crate::app_paths::data_dir();
    std::fs::create_dir_all(&dst).map_err(|e| e.to_string())?;

    let mut count = 0;
    for entry in std::fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_file() {
            std::fs::copy(&from, &to).map_err(|e| e.to_string())?;
            count += 1;
        }
    }

    Ok(format!("imported {count} files to {}", dst.display()))
}

#[tauri::command]
pub async fn check_for_updates() -> Result<String, String> {
    tokio::task::spawn_blocking(|| {
        let url = "https://api.github.com/repos/3134q108-del/capture2text-pro-rs/releases/latest";
        let client = reqwest::blocking::Client::new();
        let resp = client
            .get(url)
            .header("User-Agent", "Capture2TextPro")
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .map_err(|e| e.to_string())?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok("no_release".to_string());
        }
        if !resp.status().is_success() {
            return Err(format!("status {}", resp.status()));
        }

        let json: serde_json::Value = resp.json().map_err(|e| e.to_string())?;
        let tag = json
            .get("tag_name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        Ok(tag.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
