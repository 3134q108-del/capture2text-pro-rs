mod capture;
mod commands;
mod drag_overlay;
mod error;
mod hotkey;
mod overlay;
pub mod leptonica;
pub mod mouse_hook;
pub mod vlm;
pub use crate::capture::preprocess;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .setup(|_app| {
            match vlm::check_health() {
                vlm::HealthStatus::Healthy => {
                    println!("[vlm] ollama health: OK");
                }
                vlm::HealthStatus::OllamaDown => {
                    eprintln!("[vlm] ollama health: daemon not reachable (is 'ollama serve' running?)");
                }
                vlm::HealthStatus::ModelMissing { model } => {
                    eprintln!(
                        "[vlm] ollama health: model '{}' not found (run: ollama pull {})",
                        model, model
                    );
                }
                vlm::HealthStatus::Unknown(msg) => {
                    eprintln!("[vlm] ollama health: unknown status ({})", msg);
                }
            }
            vlm::init_worker();
            overlay::init()?;
            drag_overlay::init()?;
            capture::start_worker()?;
            hotkey::install()?;
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![commands::files::read_file])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            eprintln!("[shutdown] begin");
            eprintln!("[shutdown] hotkey");
            hotkey::shutdown();
            eprintln!("[shutdown] capture");
            capture::shutdown_worker();
            eprintln!("[shutdown] drag_overlay");
            drag_overlay::shutdown();
            eprintln!("[shutdown] overlay");
            overlay::shutdown();
            eprintln!("[shutdown] done");
        }
    });
}

