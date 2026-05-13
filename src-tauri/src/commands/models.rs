use serde::Serialize;
use std::fs;
use std::path::PathBuf;

use crate::llama_runtime::downloader;
use crate::llama_runtime::manifest::{ModelId, ModelSpec, ALL_MODELS};

fn models_dir() -> Result<PathBuf, String> {
    let dir = crate::app_paths::data_dir().join("models");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn model_files(spec: &ModelSpec) -> Result<(PathBuf, PathBuf), String> {
    let dir = models_dir()?;
    Ok((dir.join(spec.gguf_filename()), dir.join(spec.mmproj_filename())))
}

fn is_downloaded(spec: &ModelSpec) -> bool {
    if let Ok((gguf, mmproj)) = model_files(spec) {
        gguf.exists() && mmproj.exists()
    } else {
        false
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    pub size_mb: u32,
    pub supported_lang_codes: Vec<String>,
    pub downloaded: bool,
    pub active: bool,
}

#[tauri::command]
pub fn get_models_list() -> Vec<ModelInfo> {
    let active = crate::window_state::active_model();
    ALL_MODELS
        .iter()
        .map(|id| {
            let spec = id.spec();
            ModelInfo {
                id: serde_json::to_string(id)
                    .unwrap_or_default()
                    .trim_matches('"')
                    .to_string(),
                display_name: spec.display_name.to_string(),
                size_mb: spec.size_mb,
                supported_lang_codes: spec
                    .supported_lang_codes
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                downloaded: is_downloaded(spec),
                active: Some(*id) == active,
            }
        })
        .collect()
}

#[tauri::command]
pub fn get_active_model() -> Option<String> {
    crate::window_state::active_model().map(|id| {
        serde_json::to_string(&id)
            .unwrap_or_default()
            .trim_matches('"')
            .to_string()
    })
}

#[tauri::command]
pub fn get_annotator_mode() -> bool {
    crate::window_state::annotator_mode()
}

#[tauri::command]
pub fn set_annotator_mode(value: bool) -> Result<(), String> {
    crate::window_state::set_annotator_mode(value);
    Ok(())
}

fn parse_model_id(s: &str) -> Result<ModelId, String> {
    serde_json::from_str::<ModelId>(&format!("\"{}\"", s))
        .map_err(|e| format!("invalid model id: {} ({})", s, e))
}

#[tauri::command]
pub fn set_active_model(id: String) -> Result<(), String> {
    let model_id = parse_model_id(&id)?;
    let spec = model_id.spec();
    if !is_downloaded(spec) {
        return Err(format!("model {} not downloaded", spec.display_name));
    }

    crate::window_state::set_active_model(Some(model_id));
    if let Some(app) = crate::app_handle::get() {
        use tauri::Emitter;
        let _ = app.emit("active-model-changed", &id);
    }

    crate::llama_runtime::supervisor::restart_with_model(model_id)
        .map_err(|e| format!("failed to restart llama-server: {}", e))?;

    Ok(())
}

#[tauri::command]
pub fn delete_model(id: String) -> Result<(), String> {
    let model_id = parse_model_id(&id)?;
    let spec = model_id.spec();
    let (gguf, mmproj) = model_files(spec)?;
    let _ = fs::remove_file(&gguf);
    let _ = fs::remove_file(&mmproj);
    if crate::window_state::active_model() == Some(model_id) {
        crate::window_state::set_active_model(None);
    }
    if let Some(app) = crate::app_handle::get() {
        use tauri::Emitter;
        let _ = app.emit("model-deleted", &id);
    }
    Ok(())
}

#[tauri::command]
pub async fn download_model(id: String) -> Result<(), String> {
    let model_id = parse_model_id(&id)?;
    let spec = model_id.spec();
    let (gguf_path, mmproj_path) = model_files(spec)?;
    let id_for_progress = id.clone();

    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let spec = model_id.spec();

        let id_clone = id_for_progress.clone();
        downloader::download_file_with_progress(spec.gguf_url, &gguf_path, move |downloaded, total| {
            if let Some(app) = crate::app_handle::get() {
                use tauri::Emitter;
                let _ = app.emit(
                    "model-download-progress",
                    serde_json::json!({
                        "model_id": id_clone,
                        "file": "gguf",
                        "downloaded": downloaded,
                        "total": total,
                    }),
                );
            }
        })?;

        let id_clone = id_for_progress.clone();
        downloader::download_file_with_progress(spec.mmproj_url, &mmproj_path, move |downloaded, total| {
            if let Some(app) = crate::app_handle::get() {
                use tauri::Emitter;
                let _ = app.emit(
                    "model-download-progress",
                    serde_json::json!({
                        "model_id": id_clone,
                        "file": "mmproj",
                        "downloaded": downloaded,
                        "total": total,
                    }),
                );
            }
        })?;

        if let Some(app) = crate::app_handle::get() {
            use tauri::Emitter;
            let _ = app.emit("model-download-complete", &id_for_progress);
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())??;

    Ok(())
}
