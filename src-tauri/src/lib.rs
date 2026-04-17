mod capture;
mod commands;
mod error;
mod hotkey;
pub mod leptonica;
pub use crate::capture::preprocess;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|_app| {
            capture::start_worker()?;
            hotkey::install()?;
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![commands::files::read_file])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

