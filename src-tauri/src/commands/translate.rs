use crate::vlm::{self, TargetLang};

#[tauri::command]
pub fn retranslate(text: String) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("empty text".to_string());
    }

    vlm::try_submit_text(text, TargetLang::Chinese, "Retrans");
    Ok(())
}
