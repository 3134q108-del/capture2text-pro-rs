use tauri::{Emitter, Manager};

mod capture;
mod commands;
mod drag_overlay;
mod error;
mod hotkey;
mod overlay;
mod output_lang;
mod scenarios;
mod tray;
mod tts;
pub mod leptonica;
pub mod mouse_hook;
pub mod vlm;
pub use crate::capture::preprocess;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            let _ = crate::commands::result_window::show_settings_window(app.clone());
        }))
        .setup(|app| {
            scenarios::init_runtime()?;
            output_lang::init_runtime()?;
            tts::init_config_runtime().map_err(std::io::Error::other)?;
            let health = vlm::check_health();
            match &health {
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
            if let Some(warning) = health.to_warning() {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    if let Some(window) = app_handle.get_webview_window("settings") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                    let _ = app_handle.emit_to("settings", "health-warning", warning);
                });
            }
            let app_handle = app.handle().clone();
            vlm::init_worker(app_handle);
            tray::install(&app.handle().clone())?;
            overlay::init()?;
            drag_overlay::init()?;
            capture::start_worker()?;
            hotkey::install()?;
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::files::read_file,
            commands::result_window::show_result_window,
            commands::result_window::hide_result_window,
            commands::result_window::show_settings_window,
            commands::result_window::hide_settings_window,
            commands::result_window::get_latest_vlm_state,
            commands::scenarios::list_scenarios,
            commands::scenarios::save_scenario,
            commands::scenarios::delete_scenario,
            commands::scenarios::get_active_scenario,
            commands::scenarios::set_active_scenario,
            commands::tts::speak,
            commands::tts::stop_speaking,
            commands::tts::list_tts_voices,
            commands::tts::get_tts_config,
            commands::tts::set_tts_voice,
            commands::translate::retranslate
        ])
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

