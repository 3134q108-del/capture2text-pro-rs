use serde::Serialize;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize)]
pub struct VoicePresetInfo {
    pub id: String,
    pub display_name: String,
}

#[tauri::command]
pub fn speak(app: AppHandle, target: String, text: String, lang: String) -> Result<(), String> {
    eprintln!(
        "[tts] speak called target={} text_len={} lang={}",
        target,
        text.len(),
        lang
    );
    if text.trim().is_empty() {
        eprintln!("[tts] speak: empty text, skip");
        return Ok(());
    }

    std::thread::spawn(move || {
        let t0 = std::time::Instant::now();
        eprintln!("[tts] synth start target={} lang={}", target, lang);
        match crate::tts::synthesize_for_active_voice(&text, &lang) {
            Ok(wav) => {
                // === DEBUG dump WAV ===
                let dump_dir = dirs::data_local_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join("Capture2TextPro")
                    .join("tts_debug");
                let _ = std::fs::create_dir_all(&dump_dir);
                let ts = chrono::Local::now().format("%Y%m%d_%H%M%S");
                let dump_path = dump_dir.join(format!("{}_{}.wav", ts, target));
                if let Err(e) = std::fs::write(&dump_path, &wav) {
                    eprintln!("[tts] dump failed: {e}");
                } else {
                    eprintln!("[tts] dump wav -> {}", dump_path.display());
                }
                // === /DEBUG ===

                let synth_ms = t0.elapsed().as_millis();
                eprintln!(
                    "[tts] synth done target={} wav_bytes={} in {}ms",
                    target,
                    wav.len(),
                    synth_ms
                );
                let t1 = std::time::Instant::now();
                if let Err(err) = crate::tts::play_wav(&wav) {
                    eprintln!("[tts] play failed target={} err={err}", target);
                } else {
                    eprintln!(
                        "[tts] play done target={} in {}ms",
                        target,
                        t1.elapsed().as_millis()
                    );
                }
                let _ = app.emit("tts-done", serde_json::json!({ "target": target }));
            }
            Err(err) => {
                eprintln!(
                    "[tts] synth FAILED target={} in {}ms err={err}",
                    target,
                    t0.elapsed().as_millis()
                );
                let _ = app.emit(
                    "tts-done",
                    serde_json::json!({ "target": target, "error": err }),
                );
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub fn is_tts_cached(_text: String, _lang: String) -> bool {
    true
}

#[tauri::command]
pub fn stop_speaking() -> Result<(), String> {
    crate::tts::stop_current();
    Ok(())
}

#[tauri::command]
pub fn list_voice_presets() -> Vec<VoicePresetInfo> {
    crate::qwen_tts::VoicePreset::all()
        .iter()
        .map(|p| VoicePresetInfo {
            id: p.as_str().to_string(),
            display_name: p.display_name().to_string(),
        })
        .collect()
}

#[tauri::command]
pub fn set_active_preset(id: String) -> Result<(), String> {
    if crate::qwen_tts::VoicePreset::from_str(&id).is_none() {
        return Err("unknown preset".to_string());
    }
    crate::window_state::set_speech_active_preset(id);
    Ok(())
}

#[tauri::command]
pub fn preview_preset(id: String, text: String, lang: String) -> Result<(), String> {
    let preset =
        crate::qwen_tts::VoicePreset::from_str(&id).ok_or_else(|| "unknown preset".to_string())?;
    if text.trim().is_empty() {
        return Ok(());
    }

    std::thread::spawn(move || match crate::qwen_tts::synthesize(&text, preset, &lang) {
        Ok(wav) => {
            if let Err(err) = crate::tts::play_wav(&wav) {
                eprintln!("[tts] preview play failed: {err}");
            }
        }
        Err(err) => {
            eprintln!("[tts] preview synthesize failed: {err}");
        }
    });
    Ok(())
}
