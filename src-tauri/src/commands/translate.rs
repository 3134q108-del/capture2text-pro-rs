use crate::output_lang;
use crate::vlm::{self, TargetLang};

#[tauri::command]
pub fn retranslate(text: String) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("empty text".to_string());
    }

    let target_lang = if output_lang::current() == "en" {
        TargetLang::English
    } else {
        TargetLang::Chinese
    };

    vlm::try_submit_text(text, target_lang, "Retrans");
    Ok(())
}
