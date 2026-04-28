pub mod downloader;
pub mod manifest;
pub mod supervisor;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

pub use manifest::ModelId;

pub const LLAMA_CPP_TAG: &str = "b8870";

static ACTIVE_MODEL: OnceLock<Mutex<Option<ModelId>>> = OnceLock::new();

pub fn active_model() -> Option<ModelId> {
    ACTIVE_MODEL
        .get()
        .and_then(|slot| slot.lock().ok().and_then(|model| *model))
}

pub fn bootstrap(default_model: ModelId) -> Result<(), String> {
    cleanup_legacy_model_files();
    ensure_binary_installed()?;
    ensure_model_installed(&default_model)?;
    supervisor::spawn_for(&default_model)?;
    set_active_model(Some(default_model));
    Ok(())
}

pub fn switch_model(target: ModelId) -> Result<(), String> {
    if active_model() == Some(target) {
        return Ok(());
    }
    ensure_model_installed(&target)?;
    supervisor::stop();
    supervisor::spawn_for(&target)?;
    set_active_model(Some(target));
    Ok(())
}

pub fn ensure_model_for_lang(lang: &str) -> Result<(), String> {
    let target = manifest::ModelId::for_lang(lang);
    if active_model() == Some(target) {
        return Ok(());
    }
    eprintln!(
        "[llama-runtime] switching model for lang={} target={:?}",
        lang, target
    );
    switch_model(target)
}

pub fn app_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Capture2TextPro")
}

fn set_active_model(model: Option<ModelId>) {
    let slot = ACTIVE_MODEL.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = slot.lock() {
        *guard = model;
    }
}

fn ensure_binary_installed() -> Result<(), String> {
    let bin_dir = app_dir().join("bin");
    if !bin_dir.join("llama-server.exe").exists() {
        downloader::download_llama_binary(&bin_dir)?;
    }
    Ok(())
}

fn ensure_all_models_installed() -> Result<(), String> {
    for model in manifest::ALL_MODELS {
        ensure_model_installed(&model)?;
    }
    Ok(())
}

fn cleanup_legacy_model_files() {
    let model_dir = app_dir().join("models");
    let legacy_files: [&str; 0] = [];

    for name in legacy_files {
        let path = model_dir.join(name);
        if !path.exists() {
            continue;
        }

        if let Err(err) = fs::remove_file(&path) {
            eprintln!("[llama-runtime] cleanup legacy {} failed: {}", name, err);
        } else {
            eprintln!("[llama-runtime] cleanup legacy {} removed", name);
        }
    }
}

fn ensure_model_installed(id: &ModelId) -> Result<(), String> {
    let model_dir = app_dir().join("models");
    let spec = manifest::lookup(id).ok_or_else(|| "unknown model id".to_string())?;
    let targets = [
        (spec.gguf_url, spec.gguf_filename()),
        (spec.mmproj_url, spec.mmproj_filename()),
    ];
    for (url, filename) in targets {
        let target = model_dir.join(filename);
        ensure_file_complete(url, &target)?;
    }
    Ok(())
}

fn ensure_file_complete(url: &str, target: &Path) -> Result<(), String> {
    if !target.exists() {
        return downloader::download_file(url, target);
    }

    let local_size = fs::metadata(target).map_err(|e| e.to_string())?.len();
    let remote_size = head_content_length(url)?;
    if remote_size > 0 && local_size != remote_size {
        eprintln!(
            "[llama-runtime] size mismatch for {}: local={} remote={}, redownloading",
            target.display(),
            local_size,
            remote_size
        );
        fs::remove_file(target).map_err(|e| e.to_string())?;
        return downloader::download_file(url, target);
    }

    Ok(())
}

fn head_content_length(url: &str) -> Result<u64, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let response = client.head(url).send().map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "HEAD {} failed with status {}",
            url,
            response.status()
        ));
    }
    Ok(response.content_length().unwrap_or(0))
}
