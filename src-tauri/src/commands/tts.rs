use serde::Serialize;
use tauri::AppHandle;

#[derive(Debug, Clone, Serialize)]
pub struct VoicePresetInfo {
    pub id: String,
    pub label: String,
    pub lang: String,
}

#[tauri::command]
pub fn speak(_app: AppHandle, _target: String, _text: String, _lang: String) -> Result<(), String> {
    Err("Azure TTS 尚未設定（T52 整合中）".to_string())
}

#[tauri::command]
pub fn is_tts_cached(_text: String, _lang: String) -> bool {
    false
}

#[tauri::command]
pub fn stop_speaking() -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn list_voice_presets() -> Vec<VoicePresetInfo> {
    Vec::new()
}

#[tauri::command]
pub fn set_active_preset(_id: String) -> Result<(), String> {
    Err("Azure TTS 尚未設定（T52 整合中）".to_string())
}

#[tauri::command]
pub fn preview_preset(_id: String, _text: String, _lang: String) -> Result<(), String> {
    Err("Azure TTS 尚未設定（T52 整合中）".to_string())
}
