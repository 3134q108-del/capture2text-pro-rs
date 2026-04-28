use serde::Serialize;
use std::collections::HashMap;
use tauri::{AppHandle, State};

use crate::azure_tts::runtime::TtsRuntime;
use crate::azure_tts::{AzureProvider, TtsProvider, Voice};

#[derive(Debug, Clone, Serialize)]
pub struct AzureCredentialsStatus {
    pub configured: bool,
    pub region: Option<String>,
}

#[tauri::command]
pub async fn save_azure_credentials(key: String, region: String) -> Result<(), String> {
    let key = key.trim();
    let region = normalize_region(&region)?;
    if key.is_empty() {
        return Err("Azure subscription key is required".to_string());
    }
    crate::azure_tts::keyring::save_key(key).map_err(|err| err.to_string())?;
    crate::window_state::set_azure_region(Some(region));
    Ok(())
}

#[tauri::command]
pub fn get_azure_credentials_status() -> AzureCredentialsStatus {
    let configured = match crate::azure_tts::keyring::has_key() {
        Ok(value) => value,
        Err(err) => {
            eprintln!("[azure-tts] keyring status failed: {err}");
            false
        }
    };
    AzureCredentialsStatus {
        configured,
        region: crate::window_state::azure_region(),
    }
}

#[tauri::command]
pub fn delete_azure_credentials() -> Result<(), String> {
    crate::azure_tts::keyring::delete_key().map_err(|err| err.to_string())?;
    crate::window_state::set_azure_region(None);
    Ok(())
}

#[tauri::command]
pub async fn test_azure_connection() -> Result<(), String> {
    provider_from_config()?
        .test_connection()
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn list_azure_voices(lang: String) -> Result<Vec<Voice>, String> {
    provider_from_config()?
        .list_voices(&lang)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_voice_routing() -> HashMap<String, String> {
    crate::window_state::azure_voice_map()
}

#[tauri::command]
pub fn set_voice_routing(lang: String, voice_id: String) -> Result<(), String> {
    let lang = lang.trim();
    let voice_id = voice_id.trim();
    if lang.is_empty() {
        return Err("language is required".to_string());
    }
    if voice_id.is_empty() {
        crate::window_state::clear_azure_voice_for_lang(lang);
    } else {
        crate::window_state::set_azure_voice_for_lang(lang.to_string(), voice_id.to_string());
    }
    Ok(())
}

#[tauri::command]
pub async fn preview_voice(
    _app: AppHandle,
    state: State<'_, TtsRuntime>,
    lang: String,
    voice_id: String,
) -> Result<(), String> {
    if voice_id.trim().is_empty() {
        return Err("voice id is required".to_string());
    }

    if let Ok(mut guard) = state.current_task.lock() {
        if let Some(handle) = guard.take() {
            handle.abort();
            drop(handle);
        }
    }
    state.playback.stop();

    let playback = state.playback.clone();
    let current_task = state.current_task.clone();
    let handle = tokio::spawn(async move {
        let result = async {
            if let Some(bytes) = crate::azure_tts::preview_cache::read_cached(&voice_id) {
                playback.play(bytes)?;
                return Ok::<(), String>(());
            }

            let provider = provider_from_config()?;
            let phrase = preview_text_for_lang(&lang);
            let bytes = provider
                .synthesize(phrase, &voice_id, 1.0)
                .await
                .map_err(|err| err.to_string())?;
            if let Err(err) = crate::azure_tts::preview_cache::write_cache(&voice_id, &bytes) {
                eprintln!("[azure-tts] preview cache write failed voice={voice_id}: {err}");
            }
            playback.play(bytes)?;
            Ok::<(), String>(())
        }
        .await;

        if let Err(err) = result {
            eprintln!("[azure-tts] preview failed voice={voice_id}: {err}");
        }

        if let Ok(mut guard) = current_task.lock() {
            let _ = guard.take();
        }
    });

    let mut guard = state
        .current_task
        .lock()
        .map_err(|_| "tts task lock poisoned".to_string())?;
    *guard = Some(handle);
    Ok(())
}

#[tauri::command]
pub fn get_speech_rate() -> f32 {
    crate::window_state::azure_speech_rate()
}

#[tauri::command]
pub fn set_speech_rate(rate: f32) -> Result<(), String> {
    crate::window_state::set_azure_speech_rate(rate);
    Ok(())
}

fn provider_from_config() -> Result<AzureProvider, String> {
    let region = crate::window_state::azure_region()
        .ok_or_else(|| crate::tts::TtsError::NotConfigured.to_string())?;
    let key = crate::azure_tts::keyring::get_key()
        .map_err(|err| err.to_string())?
        .ok_or_else(|| crate::tts::TtsError::NotConfigured.to_string())?;
    Ok(AzureProvider::new(region, key))
}

fn normalize_region(region: &str) -> Result<String, String> {
    let normalized = region.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        Err("Azure region is required".to_string())
    } else {
        Ok(normalized)
    }
}

fn preview_text_for_lang(lang: &str) -> &'static str {
    match lang {
        "zh-TW" => "春暖花開，這段語音用來確認聲音是否自然清楚。",
        "zh-CN" => "春暖花开，这段语音用来确认声音是否自然清楚。",
        "en-US" | "en-GB" | "en" => {
            "The quick brown fox jumps over the lazy dog. She sells seashells by the seashore."
        }
        "ja-JP" | "ja" => "今日は良い天気です。この音声で声の雰囲気を確認します。",
        "ko-KR" | "ko" => "오늘은 날씨가 좋습니다. 이 음성으로 목소리를 확인합니다.",
        "de-DE" | "de" => {
            "Heute ist ein wunderschöner Tag. Fünf flinke Füchse springen über den Zaun."
        }
        "fr-FR" | "fr" => "Bonjour, le ciel est bleu. Comment allez-vous aujourd'hui ?",
        _ => "Hello, this is a voice preview.",
    }
}
