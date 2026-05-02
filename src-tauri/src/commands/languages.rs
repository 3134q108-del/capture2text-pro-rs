use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct LanguagePayload {
    pub code: String,
    pub native_name: String,
    pub english_name: String,
    pub tier: String,
}

#[tauri::command]
pub fn get_languages() -> Vec<LanguagePayload> {
    crate::languages::all()
        .iter()
        .map(|lang| LanguagePayload {
            code: lang.code.as_str().to_string(),
            native_name: lang.native_name.to_string(),
            english_name: lang.english_name.to_string(),
            tier: match lang.tier {
                crate::languages::Tier::S => "S",
                crate::languages::Tier::A => "A",
                crate::languages::Tier::B => "B",
                crate::languages::Tier::C => "C",
            }
            .to_string(),
        })
        .collect()
}

#[tauri::command]
pub fn get_enabled_langs() -> Vec<String> {
    crate::window_state::enabled_langs()
}

#[tauri::command]
pub fn get_translation_mode() -> String {
    match crate::window_state::translation_mode() {
        crate::window_state::TranslationMode::Smart => "Smart".to_string(),
        crate::window_state::TranslationMode::Direct => "Direct".to_string(),
    }
}

#[tauri::command]
pub fn set_translation_mode(mode: String) -> Result<(), String> {
    let parsed = match mode.as_str() {
        "Smart" => crate::window_state::TranslationMode::Smart,
        "Direct" => crate::window_state::TranslationMode::Direct,
        other => return Err(format!("invalid translation mode: {}", other)),
    };
    crate::window_state::set_translation_mode(parsed);
    Ok(())
}

#[tauri::command]
pub fn set_language_preferences(
    native_lang: String,
    target_lang: String,
    enabled_langs: Vec<String>,
) -> Result<(), String> {
    crate::window_state::set_language_preferences(native_lang, target_lang, enabled_langs)
}
