pub mod downloader;
pub mod manifest;
pub mod supervisor;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

pub use manifest::ModelId;

pub const LLAMA_CPP_TAG: &str = "b9994";
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
    let _guard = SWITCH_LOCK
        .lock()
        .map_err(|e| format!("switch_lock poisoned: {e}"))?;
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

pub fn needs_switch_for_lang(lang: &str) -> bool {
    should_switch_for_lang_impl(active_model(), lang, is_model_downloaded)
}

pub fn ensure_model_for_lang(lang: &str) -> Result<(), String> {
    if !any_model_downloaded() {
        return Err("no_model: 請先下載並選擇 AI 模型".to_string());
    }
    // 尊重 user 選擇：當前 active model 支援該語言就不切換
    if let Some(current) = active_model() {
        if current.supports_lang(lang) {
            return Ok(());
        }
    }

    // 在已下載模型中找第一個支援該語言者
    let downloaded_target = manifest::ALL_MODELS
        .iter()
        .find(|id| is_model_downloaded(id) && id.supports_lang(lang))
        .copied();

    if let Some(target) = downloaded_target {
        eprintln!(
            "[llama-runtime] active model does not support lang={}, switching to downloaded {:?}",
            lang, target
        );
        switch_model(target)
    } else {
        // 沒有已下載模型支援：保留目前模型，避免回報未下載錯誤
        eprintln!(
            "[llama-runtime] no downloaded model supports lang={}, keeping current",
            lang
        );
        Ok(())
    }
}

fn should_switch_for_lang_impl<F>(
    current: Option<ModelId>,
    lang: &str,
    mut is_downloaded: F,
) -> bool
where
    F: FnMut(&ModelId) -> bool,
{
    if let Some(current) = current {
        if current.supports_lang(lang) {
            return false;
        }
    }

    manifest::ALL_MODELS
        .iter()
        .any(|id| is_downloaded(id) && id.supports_lang(lang))
}

pub fn any_model_downloaded() -> bool {
    manifest::ALL_MODELS.iter().any(is_model_downloaded)
}

pub fn app_dir() -> PathBuf {
    crate::app_paths::data_dir()
}

fn ensure_binary_installed() -> Result<(), String> {
    let bin_dir = app_dir().join("bin");
    downloader::ensure_llama_binary_installed(&bin_dir)?;
    Ok(())
}

#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switch_decision_checks_current_and_downloaded_support() {
        let only_8b_downloaded = |id: &ModelId| matches!(id, ModelId::Qwen3Vl8bInstruct);

        assert!(!should_switch_for_lang_impl(
            Some(ModelId::Qwen3Vl8bInstruct),
            "en-US",
            only_8b_downloaded
        ));
        assert!(should_switch_for_lang_impl(
            Some(ModelId::Qwen3Vl2bInstruct),
            "vi-VN",
            only_8b_downloaded
        ));
        assert!(!should_switch_for_lang_impl(
            Some(ModelId::Qwen3Vl2bInstruct),
            "vi-VN",
            |_| false
        ));
        assert!(should_switch_for_lang_impl(
            None,
            "en-US",
            only_8b_downloaded
        ));
    }
}
