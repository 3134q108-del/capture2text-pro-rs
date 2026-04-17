use crate::error::CommandError;

#[tauri::command]
pub fn read_file(path: String) -> Result<String, CommandError> {
    std::fs::read_to_string(&path).map_err(|e| CommandError::Io(e.to_string()))
}
