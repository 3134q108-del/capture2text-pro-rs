use crate::tts::{self, TtsConfig, TtsVoiceOption};
use tauri::{AppHandle, Emitter};

#[tauri::command]
pub fn speak(app: AppHandle, text: String, lang: String) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("empty text".to_string());
    }

    std::thread::spawn(move || {
        let result = (|| -> Result<(), String> {
            let voice_code = tts::current_voice_for_lang(lang.as_str());
            let bytes =
                tts::synthesize_with_voice(&text, &voice_code).map_err(|err| err.to_string())?;
            tts::play_mp3(&bytes).map_err(|err| err.to_string())?;
            Ok(())
        })();

        if let Err(err) = result {
            eprintln!("[tts] speak worker failed: {}", err);
        }
        let _ = app.emit("tts-done", ());
    });

    Ok(())
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
