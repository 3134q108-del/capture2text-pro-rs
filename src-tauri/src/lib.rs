mod capture;
mod commands;
mod error;
mod hotkey;
mod overlay;
pub mod leptonica;
pub use crate::capture::preprocess;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let result = tauri::Builder::default()
        .setup(|_app| {
            overlay::init()?;
            capture::start_worker()?;
            hotkey::install()?;
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![commands::files::read_file])
        .run(tauri::generate_context!());

    overlay::shutdown();

    result.expect("error while running tauri application");
}

