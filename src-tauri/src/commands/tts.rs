use serde::Serialize;
use tauri::{Emitter, State};

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
    state: State<'_, TtsRuntime>,
    target: String,
    text: String,
    lang: String,
) -> Result<(), String> {
    if text.trim().is_empty() {
        return Ok(());
    }

    abort_current_task(state.inner());
    stop_player_silent(state.inner());

    let region = crate::window_state::azure_region().ok_or_else(not_configured_message)?;
    let key = crate::azure_tts::keyring::get_key()
        .map_err(|err| err.to_string())?
        .ok_or_else(not_configured_message)?;
    let actual_lang = match target.as_str() {
        "original" => crate::vlm::active_src_lang().unwrap_or_else(|| lang.clone()),
        "translated" => crate::output_lang::current(),
        _ => lang.clone(),
    };
    let normalized_lang = normalize_lang(&actual_lang);
    let mut voice_id = crate::window_state::azure_voice_map()
        .get(normalized_lang)
        .cloned()
        .unwrap_or_else(|| default_voice_for_lang(normalized_lang).to_string());
    if crate::window_state::azure_billing_tier() == crate::window_state::BillingTier::F0
        && crate::azure_tts::usage::is_hd_voice(&voice_id)
    {
        voice_id = default_voice_for_lang(normalized_lang).to_string();
    }
    eprintln!(
        "[tts] speak target={target} lang={actual_lang} normalized={normalized_lang} voice_id={voice_id}"
    );
    let rate = crate::window_state::azure_speech_rate();
    let volume = crate::window_state::azure_speech_volume();
    let app = state.inner().app.clone();
    let playback = state.inner().playback.clone();
    let current_task = state.inner().current_task.clone();

    let handle = tokio::spawn(async move {
        let result = async {
            let mp3 = if let Some(bytes) = crate::azure_tts::speak_cache::read_cached(
                &voice_id, &text, rate, volume,
            ) {
                bytes
            } else {
                let provider = AzureProvider::new(region, key);
                let bytes = provider
                    .synthesize(&text, &voice_id, rate, volume)
                    .await
                    .map_err(|err| err.to_string())?;
                if let Err(err) =
                    crate::azure_tts::speak_cache::write_cache(
                        &voice_id, &text, rate, volume, &bytes,
                    )
                {
                    eprintln!("[azure-tts] speak cache write failed voice={voice_id}: {err}");
                }
                bytes
            };
            let _ = app.emit("tts-synthesized", serde_json::json!({ "target": target }));
            playback.play_for_target(target.clone(), mp3)?;
            Ok::<(), String>(())
        }
        .await;

        if let Err(err) = result {
            eprintln!("[tts] speak failed target={target} err={err}");
            let _ = app.emit(
                "tts-done",
                serde_json::json!({ "target": target, "error": err }),
            );
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

fn stop_player_silent(state: &TtsRuntime) {
    state.playback.stop_silent();
}

fn default_voice_for_lang(lang: &str) -> &'static str {
    crate::languages::by_code(normalize_lang(lang))
        .map(|lang| lang.default_voice_id)
        .unwrap_or("en-US-AvaNeural")
}

fn normalize_lang(lang: &str) -> &str {
    let trimmed = lang.trim();
    match trimmed.to_ascii_lowercase().as_str() {
        "zh" | "zh-tw" => "zh-TW",
        "zh-cn" => "zh-CN",
        "en" | "en-us" | "en-gb" => "en-US",
        "ja" | "ja-jp" => "ja-JP",
        "ko" | "ko-kr" => "ko-KR",
        "fr" | "fr-fr" => "fr-FR",
        "de" | "de-de" => "de-DE",
        "es" | "es-es" => "es-ES",
        "pt" | "pt-pt" => "pt-PT",
        "it" | "it-it" => "it-IT",
        "ru" | "ru-ru" => "ru-RU",
        "vi" | "vi-vn" => "vi-VN",
        "ar" | "ar-sa" => "ar-SA",
        "id" | "id-id" => "id-ID",
        "th" | "th-th" => "th-TH",
        "hi" | "hi-in" => "hi-IN",
        "el" | "el-gr" => "el-GR",
        "he" | "he-il" => "he-IL",
        "tr" | "tr-tr" => "tr-TR",
        "pl" | "pl-pl" => "pl-PL",
        "nl" | "nl-nl" => "nl-NL",
        "uk" | "uk-ua" => "uk-UA",
        "cs" | "cs-cz" => "cs-CZ",
        "sv" | "sv-se" => "sv-SE",
        "da" | "da-dk" => "da-DK",
        "no" | "no-no" => "no-NO",
        "fi" | "fi-fi" => "fi-FI",
        "hu" | "hu-hu" => "hu-HU",
        "ro" | "ro-ro" => "ro-RO",
        "bg" | "bg-bg" => "bg-BG",
        "ms" | "ms-my" => "ms-MY",
        "tl" | "fil" | "fil-ph" => "fil-PH",
        _ => trimmed,
    }
}

fn not_configured_message() -> String {
    "Azure TTS is not configured. Set API key and region in Settings > Speech.".to_string()
}
