use tauri::Manager;

mod capture;
mod clipboard;
mod commands;
mod drag_overlay;
mod error;
mod hotkey;
mod llama_runtime;
#[allow(dead_code)]
mod ollama_boot;
mod overlay;
mod output_lang;
mod app_handle;
mod azure_tts;
mod scenarios;
mod tray;
mod tts;
mod window_state;
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
            app.manage(crate::azure_tts::runtime::TtsRuntime::new(&app.handle()));
            crate::app_handle::set(app.handle().clone());
            scenarios::init_runtime()?;
            output_lang::init_runtime()?;
            window_state::init_runtime();
            let app_handle_for_bootstrap = app.handle().clone();
            std::thread::spawn(move || {
                if let Err(err) = crate::llama_runtime::bootstrap(
                    crate::llama_runtime::manifest::ModelId::Qwen3Vl8bInstruct,
                ) {
                    eprintln!("[llama-runtime] bootstrap failed: {err}");
                    use tauri::Emitter;
                    let _ = app_handle_for_bootstrap.emit("llm-setup-failed", err.clone());
                    let _ = app_handle_for_bootstrap.emit_to(
                        "settings",
                        "health-warning",
                        crate::vlm::HealthWarning {
                            status: "setup-failed".to_string(),
                            message: err,
                        },
                    );
                    return;
                }

                crate::vlm::warmup();
            });
            for label in ["result", "settings"] {
                if let Some(window) = app.get_webview_window(label) {
                    eprintln!("[window] attach_close_handler label={}", label);
                    commands::result_window::attach_close_handler(window);
                }
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
            commands::result_window::get_window_state,
            commands::result_window::set_popup_topmost,
            commands::result_window::set_popup_font,
            commands::result_window::clear_popup_font,
            commands::result_window::save_popup_window_geometry,
            commands::result_window::set_save_to_clipboard,
            commands::result_window::set_clipboard_mode,
            commands::result_window::set_popup_show_enabled,
            commands::result_window::set_translate_append_to_clipboard,
            commands::result_window::set_translate_separator,
            commands::result_window::set_log_enabled,
            commands::result_window::set_log_file_path,
            commands::result_window::set_speech_enabled,
            commands::result_window::write_popup_clipboard,
            commands::result_window::check_llm_health,
            commands::result_window::check_ollama_health,
            commands::result_window::open_external_url,
            commands::result_window::export_settings,
            commands::result_window::import_settings,
            commands::result_window::check_for_updates,
            commands::output_lang::get_output_language,
            commands::output_lang::set_output_language,
            commands::scenarios::list_scenarios,
            commands::scenarios::save_scenario,
            commands::scenarios::delete_scenario,
            commands::scenarios::get_active_scenario,
            commands::scenarios::set_active_scenario,
            commands::azure_tts::save_azure_credentials,
            commands::azure_tts::get_azure_credentials_status,
            commands::azure_tts::delete_azure_credentials,
            commands::azure_tts::test_azure_connection,
            commands::azure_tts::list_azure_voices,
            commands::azure_tts::get_voice_routing,
            commands::azure_tts::set_voice_routing,
            commands::azure_tts::preview_voice,
            commands::azure_tts::get_speech_rate,
            commands::azure_tts::set_speech_rate,
            commands::azure_tts::get_azure_usage_info,
            commands::azure_tts::set_billing_tier,
            commands::azure_tts::set_neural_limit,
            commands::azure_tts::set_hd_limit,
            commands::tts::speak,
            commands::tts::is_tts_cached,
            commands::tts::stop_speaking,
            commands::tts::list_voice_presets,
            commands::tts::set_active_preset,
            commands::tts::preview_preset,
            commands::translate::retranslate
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            eprintln!("[shutdown] begin");
            eprintln!("[shutdown] llama_runtime");
            llama_runtime::supervisor::stop();
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

