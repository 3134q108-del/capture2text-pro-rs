pub mod downloader;
pub mod manifest;
pub mod supervisor;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

pub use manifest::ModelId;

pub const LLAMA_CPP_TAG: &str = "b8955";
static SWITCH_LOCK: Mutex<()> = Mutex::new(());

pub fn active_model() -> Option<ModelId> {
    crate::window_state::active_model()
}

pub fn bootstrap(default_model: ModelId) -> Result<(), String> {
    cleanup_legacy_model_files();
    ensure_binary_installed()?;
    let startup_model = active_model().unwrap_or(default_model);
    if is_model_downloaded(&startup_model) {
        supervisor::spawn_for(&startup_model)?;
        crate::window_state::set_active_model(Some(startup_model));
    } else {
        eprintln!(
            "[llama-runtime] startup model {:?} not downloaded, skip spawning",
            startup_model
        );
        crate::window_state::set_active_model(None);
    }
    Ok(())
}

pub fn switch_model(target: ModelId) -> Result<(), String> {
    let _guard = SWITCH_LOCK.lock().map_err(|e| format!("switch_lock poisoned: {e}"))?;
    if active_model() == Some(target) {
        return Ok(());
    }
    if !is_model_downloaded(&target) {
        return Err(format!(
            "model {} not downloaded",
            target.spec().display_name
        ));
    }
    supervisor::stop();
    supervisor::spawn_for(&target)?;
    crate::window_state::set_active_model(Some(target));
    Ok(())
}

pub fn ensure_model_for_lang(lang: &str) -> Result<(), String> {
    if !any_model_downloaded() {
        return Err("no_model: 請先下載並選擇 AI 模型".to_string());
    }
    // 尊重 user 的選擇：若當前 active model 支援該語言，不切換
    if let Some(current) = active_model() {
        if current.supports_lang(lang) {
            return Ok(());
        }
        // 當前模型不支援，fallback
        let target = manifest::ModelId::for_lang(lang);
        eprintln!(
            "[llama-runtime] active model {:?} does not support lang={}, switching to {:?}",
            current, lang, target
        );
        return switch_model(target);
    }
    // 沒有 active model（首次啟動或被刪除）用 for_lang fallback
    let target = manifest::ModelId::for_lang(lang);
    eprintln!(
        "[llama-runtime] no active model, switching for lang={} target={:?}",
        lang, target
    );
    switch_model(target)
}

pub fn any_model_downloaded() -> bool {
    manifest::ALL_MODELS.iter().any(is_model_downloaded)
}

pub fn app_dir() -> PathBuf {
    crate::app_paths::data_dir()
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

fn is_model_downloaded(id: &ModelId) -> bool {
    let model_dir = app_dir().join("models");
    let spec = id.spec();
    model_dir.join(spec.gguf_filename()).exists() && model_dir.join(spec.mmproj_filename()).exists()
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
