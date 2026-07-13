use serde::Serialize;
use std::sync::{Mutex, OnceLock};
use tauri::{Emitter, State};

use crate::azure_tts::runtime::TtsRuntime;
use crate::azure_tts::{AzureProvider, TtsProvider};

static ACTIVE_TTS_TARGET: OnceLock<Mutex<Option<String>>> = OnceLock::new();

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

    let _ = abort_current_task(state.inner());
    stop_player_silent(state.inner());

    let region = crate::window_state::azure_region().ok_or_else(not_configured_message)?;
    let key = crate::azure_tts::keyring::get_key()
        .map_err(|err| err.to_string())?
        .ok_or_else(not_configured_message)?;
    let actual_lang = resolve_speak_lang(target.as_str(), &lang, crate::vlm::active_src_lang());
    let normalized_lang = normalize_lang(&actual_lang).to_string();
    let mut voice_id = crate::window_state::azure_voice_map()
        .get(normalized_lang.as_str())
        .cloned()
        .unwrap_or_else(|| voice_for_lang(normalized_lang.as_str()).to_string());
    if crate::window_state::azure_billing_tier() == crate::window_state::BillingTier::F0
        && crate::azure_tts::usage::is_hd_voice(&voice_id)
    {
        voice_id = voice_for_lang(normalized_lang.as_str()).to_string();
    }
    eprintln!(
        "[tts] speak target={target} lang={actual_lang} normalized={normalized_lang} voice_id={voice_id}"
    );
    let rate = crate::window_state::azure_speech_rate();
    let volume = crate::window_state::azure_speech_volume();
    let app = state.inner().app.clone();
    let playback = state.inner().playback.clone();
    let current_task = state.inner().current_task.clone();
    let task_target = target.clone();

    set_active_target(Some(task_target.clone()));

    let handle = tokio::spawn(async move {
        let result = async {
            let (mp3, cached_voice_id) = synthesize_with_voice_fallback(
                region,
                key,
                &text,
                normalized_lang.as_str(),
                &voice_id,
                rate,
                volume,
            )
            .await?;
            if let Err(err) = crate::azure_tts::speak_cache::write_cache(
                &cached_voice_id,
                &text,
                rate,
                volume,
                &mp3,
            ) {
                eprintln!("[azure-tts] speak cache write failed voice={cached_voice_id}: {err}");
            }
            let _ = app.emit("tts-synthesized", serde_json::json!({ "target": target }));
            playback.play_for_target(target.clone(), mp3)?;
            Ok::<(), String>(())
        }
        .await;

        if let Err(err) = result {
            eprintln!("[tts] speak failed target={target} err={err}");
            clear_active_target_if_matches(&target);
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
    let target = abort_current_task(state.inner());
    stop_player_silent(state.inner());
    if let Some(target) = target {
        let _ = state
            .inner()
            .app
            .emit("tts-done", serde_json::json!({ "target": target }));
    }
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

fn abort_current_task(state: &TtsRuntime) -> Option<String> {
    let target = take_active_target();
    if let Ok(mut guard) = state.current_task.lock() {
        if let Some(handle) = guard.take() {
            handle.abort();
            drop(handle);
        }
    }
    target
}

fn stop_player_silent(state: &TtsRuntime) {
    state.playback.stop_silent();
}

fn active_target_store() -> &'static Mutex<Option<String>> {
    ACTIVE_TTS_TARGET.get_or_init(|| Mutex::new(None))
}

fn set_active_target(target: Option<String>) {
    if let Ok(mut guard) = active_target_store().lock() {
        *guard = target;
    }
}

fn take_active_target() -> Option<String> {
    active_target_store()
        .lock()
        .ok()
        .and_then(|mut guard| guard.take())
}

fn clear_active_target_if_matches(target: &str) {
    if let Ok(mut guard) = active_target_store().lock() {
        if guard.as_deref() == Some(target) {
            *guard = None;
        }
    }
}

fn voice_for_lang(lang: &str) -> &'static str {
    crate::languages::by_code(normalize_lang(lang))
        .map(|lang| lang.default_voice_id)
        .unwrap_or("en-US-AvaNeural")
}

fn resolve_speak_lang(target: &str, lang: &str, active_src_lang: Option<String>) -> String {
    match target {
        "original" => {
            if !lang.trim().is_empty() {
                lang.to_string()
            } else {
                active_src_lang.unwrap_or_else(|| lang.to_string())
            }
        }
        "translated" => crate::output_lang::current(),
        _ => lang.to_string(),
    }
}

fn should_retry_with_default_voice(requested_voice_id: &str, fallback_voice_id: &str) -> bool {
    requested_voice_id != fallback_voice_id
}

async fn synthesize_with_voice_fallback(
    region: String,
    key: String,
    text: &str,
    normalized_lang: &str,
    requested_voice_id: &str,
    rate: f32,
    volume: f32,
) -> Result<(Vec<u8>, String), String> {
    if let Some(bytes) =
        crate::azure_tts::speak_cache::read_cached(requested_voice_id, text, rate, volume)
    {
        return Ok((bytes, requested_voice_id.to_string()));
    }

    let provider = AzureProvider::new(region, key);

    match provider
        .synthesize(text, requested_voice_id, rate, volume)
        .await
    {
        Ok(bytes) => Ok((bytes, requested_voice_id.to_string())),
        Err(err) => {
            let default_voice_for_lang = voice_for_lang(normalized_lang);
            let fallback_voice_id = default_voice_for_lang;
            if !should_retry_with_default_voice(requested_voice_id, fallback_voice_id) {
                return Err(err.to_string());
            }

            eprintln!(
                "[tts] synthesize failed voice={requested_voice_id} lang={normalized_lang} err={err}; retrying with default voice={fallback_voice_id}"
            );
            if let Some(bytes) =
                crate::azure_tts::speak_cache::read_cached(fallback_voice_id, text, rate, volume)
            {
                return Ok((bytes, fallback_voice_id.to_string()));
            }

            let bytes = provider
                .synthesize(text, fallback_voice_id, rate, volume)
                .await
                .map_err(|fallback_err| {
                    format!(
                        "voice synthesis failed voice={requested_voice_id} fallback={fallback_voice_id} err={err}; fallback err={fallback_err}"
                    )
                })?;
            Ok((bytes, fallback_voice_id.to_string()))
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_speak_prefers_explicit_lang_over_active_src_lang() {
        assert_eq!(
            resolve_speak_lang("original", "en-US", Some("zh-TW".to_string())),
            "en-US"
        );
        assert_eq!(
            resolve_speak_lang("original", "", Some("zh-TW".to_string())),
            "zh-TW"
        );
        assert_eq!(resolve_speak_lang("original", "", None), "");
    }

    #[test]
    fn default_voices_are_region_safe_standard_neural() {
        assert_eq!(voice_for_lang("en-US"), "en-US-JennyNeural");
        assert_eq!(voice_for_lang("fr-FR"), "fr-FR-DeniseNeural");
        assert_eq!(voice_for_lang("de-DE"), "de-DE-KatjaNeural");
    }

    #[test]
    fn fallback_voice_retry_is_disabled_for_default_voice() {
        assert!(!should_retry_with_default_voice(
            "en-US-JennyNeural",
            voice_for_lang("en-US")
        ));
        assert!(should_retry_with_default_voice(
            "en-US-AvaMultilingualNeural",
            voice_for_lang("en-US")
        ));
    }
}
