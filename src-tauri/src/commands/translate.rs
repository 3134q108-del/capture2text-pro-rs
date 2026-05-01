use crate::vlm;
use crate::window_state;

#[tauri::command]
pub fn retranslate(text: String) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("empty text".to_string());
    }

    let state = window_state::get();
    vlm::try_submit_text(text, state.native_lang, state.target_lang, "Retrans");
    Ok(())
}
