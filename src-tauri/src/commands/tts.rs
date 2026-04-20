use crate::tts::{self, TtsConfig, TtsVoiceOption};
use tauri::{AppHandle, Emitter};

#[tauri::command]
pub fn speak(app: AppHandle, target: String, text: String, lang: String) -> Result<(), String> {
    if text.trim().is_empty() {
        return Ok(());
    }

    let voice_code = tts::current_voice_for_lang(lang.as_str());
    let Some(mp3) = tts::cache_get(&text, &voice_code) else {
        eprintln!(
            "[tts] cache miss voice={} text_len={} refusing synthesize (wait prefetch)",
            voice_code,
            text.len()
        );
        return Err("not-ready".to_string());
    };

    std::thread::spawn(move || {
        if let Err(err) = tts::play_mp3(&mp3) {
            eprintln!("[tts] play failed: {}", err);
        }
        let _ = app.emit("tts-done", serde_json::json!({ "target": target }));
    });

    Ok(())
}

#[tauri::command]
pub fn is_tts_cached(text: String, lang: String) -> bool {
    if text.trim().is_empty() {
        return false;
    }

    let voice_code = tts::current_voice_for_lang(lang.as_str());
    tts::cache_get(&text, &voice_code).is_some()
}

#[tauri::command]
pub fn stop_speaking() -> Result<(), String> {
    tts::stop_current();
    Ok(())
}

#[tauri::command]
pub fn list_tts_voices() -> Vec<TtsVoiceOption> {
    tts::available_voices()
}

#[tauri::command]
pub fn get_tts_config() -> Result<TtsConfig, String> {
    tts::get_config().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn set_tts_voice(lang: String, code: String) -> Result<(), String> {
    tts::set_voice(lang.as_str(), code).map_err(|err| err.to_string())
}
