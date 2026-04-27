use crate::output_lang;
use crate::vlm::{self, TargetLang};

#[tauri::command]
pub fn retranslate(text: String) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("empty text".to_string());
    }

    let target_lang = match output_lang::current().as_str() {
        "zh-CN" => TargetLang::SimplifiedChinese,
        "en-US" => TargetLang::English,
        "ja-JP" => TargetLang::Japanese,
        "ko-KR" => TargetLang::Korean,
        "de-DE" => TargetLang::German,
        "fr-FR" => TargetLang::French,
        _ => TargetLang::TraditionalChinese,
    };

    vlm::try_submit_text(text, target_lang, "Retrans");
    Ok(())
}
