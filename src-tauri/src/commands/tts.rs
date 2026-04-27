use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::azure_tts::runtime::TtsRuntime;
use crate::azure_tts::{AzureProvider, TtsProvider};

#[derive(Debug, Clone, Serialize)]
pub struct VoicePresetInfo {
    pub id: String,
    pub label: String,
    pub lang: String,
}

#[tauri::command]
pub async fn speak(
    app: AppHandle,
    state: State<'_, TtsRuntime>,
    target: String,
    text: String,
    lang: String,
) -> Result<(), String> {
    if text.trim().is_empty() {
        return Ok(());
    }

    abort_current_task(state.inner());
    stop_player(state.inner());

    let region = crate::window_state::azure_region().ok_or_else(not_configured_message)?;
    let key = crate::azure_tts::keyring::get_key()
        .map_err(|err| err.to_string())?
        .ok_or_else(not_configured_message)?;
    let normalized_lang = normalize_lang(&lang);
    let voice_id = crate::window_state::azure_voice_map()
        .get(normalized_lang)
        .cloned()
        .unwrap_or_else(|| default_voice_for_lang(normalized_lang).to_string());
    let playback = state.inner().playback.clone();
    let current_task = state.inner().current_task.clone();

    let handle = tokio::spawn(async move {
        let provider = AzureProvider::new(region, key);
        let result = async {
            let mp3 = provider
                .synthesize(&text, &voice_id, 1.0)
                .await
                .map_err(|err| err.to_string())?;
            playback.play(mp3)?;
            Ok::<(), String>(())
        }
        .await;

        if let Err(err) = result {
            eprintln!("[tts] speak failed target={target} err={err}");
            let _ = app.emit(
                "tts-done",
                serde_json::json!({ "target": target, "error": err }),
            );
        } else {
            let _ = app.emit("tts-done", serde_json::json!({ "target": target }));
        }

        if let Ok(mut guard) = current_task.lock() {
            let _ = guard.take();
        }
    });

    let mut guard = state
        .inner()
        .current_task
        .lock()
        .map_err(|_| "tts task lock poisoned".to_string())?;
    *guard = Some(handle);
    Ok(())
}

#[tauri::command]
pub fn is_tts_cached(_text: String, _lang: String) -> bool {
    false
}

#[tauri::command]
pub fn stop_speaking(state: State<'_, TtsRuntime>) -> Result<(), String> {
    abort_current_task(state.inner());
    stop_player(state.inner());
    Ok(())
}

#[tauri::command]
pub fn list_voice_presets() -> Vec<VoicePresetInfo> {
    Vec::new()
}

#[tauri::command]
pub fn set_active_preset(_id: String) -> Result<(), String> {
    Err("Azure TTS preset selection is not implemented yet (T52 in progress)".to_string())
}

#[tauri::command]
pub fn preview_preset(_id: String, _text: String, _lang: String) -> Result<(), String> {
    Err("Azure TTS preview is not implemented yet (T52 in progress)".to_string())
}

fn abort_current_task(state: &TtsRuntime) {
    if let Ok(mut guard) = state.current_task.lock() {
        if let Some(handle) = guard.take() {
            handle.abort();
            drop(handle);
        }
    }
}

fn stop_player(state: &TtsRuntime) {
    state.playback.stop();
}

fn default_voice_for_lang(lang: &str) -> &'static str {
    match lang {
        "zh-TW" => "zh-TW-HsiaoChenNeural",
        "zh-CN" => "zh-CN-XiaoxiaoNeural",
        "en-US" | "en-GB" | "en" => "en-US-AvaMultilingualNeural",
        "ja-JP" | "ja" => "ja-JP-NanamiNeural",
        "ko-KR" | "ko" => "ko-KR-SunHiNeural",
        "de-DE" | "de" => "de-DE-SeraphinaMultilingualNeural",
        "fr-FR" | "fr" => "fr-FR-VivienneMultilingualNeural",
        _ => "en-US-AvaMultilingualNeural",
    }
}

fn normalize_lang(lang: &str) -> &str {
    match lang {
        "zh-TW" | "zh-CN" | "en-US" | "ja-JP" | "ko-KR" | "de-DE" | "fr-FR" => lang,
        "en" | "en-GB" => "en-US",
        "ja" => "ja-JP",
        "ko" => "ko-KR",
        "de" => "de-DE",
        "fr" => "fr-FR",
        _ => lang,
    }
}

fn not_configured_message() -> String {
    "Azure TTS is not configured. Set API key and region in Settings > Speech.".to_string()
}
