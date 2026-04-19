use crate::tts::{self, TtsVoice};

#[tauri::command]
pub fn speak(text: String, lang: String) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("empty text".to_string());
    }

    let voice = match lang.as_str() {
        "en" => TtsVoice::English,
        _ => TtsVoice::Chinese,
    };

    let bytes = tts::synthesize(&text, voice).map_err(|err| err.to_string())?;
    tts::play_mp3(&bytes).map_err(|err| err.to_string())?;
    Ok(())
}
