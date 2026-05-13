use crate::vlm;
use crate::window_state;

#[tauri::command]
pub fn retranslate(text: String, target_lang: Option<String>) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("empty text".to_string());
    }

    let lang = target_lang.unwrap_or_else(|| window_state::get().target_lang);
    vlm::try_submit_text(text, lang, "Retrans");
    Ok(())
}
