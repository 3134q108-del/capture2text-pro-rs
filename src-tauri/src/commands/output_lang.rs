#[tauri::command]
pub fn get_output_language() -> String {
    crate::output_lang::current()
}

#[tauri::command]
pub fn set_output_language(lang: String) -> Result<(), String> {
    crate::output_lang::set(&lang).map_err(|err| err.to_string())
}
