use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

const DEFAULT_LANG: &str = "zh-TW";

static ACTIVE_OUTPUT_LANG: OnceLock<Mutex<String>> = OnceLock::new();

pub fn init_runtime() -> io::Result<()> {
    if ACTIVE_OUTPUT_LANG.get().is_some() {
        return Ok(());
    }

    let path = storage_path()?;
    let mut lang = if path.exists() {
        let raw = fs::read_to_string(&path)?;
        sanitize(raw.trim())
    } else {
        DEFAULT_LANG.to_string()
    };
    if !available_langs().iter().any(|item| item == &lang) {
        lang = DEFAULT_LANG.to_string();
    }

    persist(&lang)?;
    let _ = ACTIVE_OUTPUT_LANG.set(Mutex::new(lang));
    Ok(())
}

pub fn current() -> String {
    let Some(slot) = ACTIVE_OUTPUT_LANG.get() else {
        return DEFAULT_LANG.to_string();
    };

    match slot.lock() {
        Ok(guard) => guard.clone(),
        Err(_) => DEFAULT_LANG.to_string(),
    }
}

pub fn available_langs() -> Vec<String> {
    vec![
        "zh-TW".to_string(),
        "zh-CN".to_string(),
        "en-US".to_string(),
        "ja-JP".to_string(),
        "ko-KR".to_string(),
        "de-DE".to_string(),
        "fr-FR".to_string(),
    ]
}

pub fn set(lang: &str) -> io::Result<()> {
    let next = sanitize(lang);
    if !available_langs().iter().any(|item| item == &next) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("language not available yet: {next}"),
        ));
    }
    persist(&next)?;

    if let Some(slot) = ACTIVE_OUTPUT_LANG.get() {
        if let Ok(mut guard) = slot.lock() {
            *guard = next.clone();
        }
    }

    if let Some(app) = crate::app_handle::get() {
        use tauri::Emitter;
        let _ = app.emit("output-language-changed", &next);
    }
    Ok(())
}

fn sanitize(lang: &str) -> String {
    match lang.trim().to_ascii_lowercase().as_str() {
        "zh-tw" | "zh" => "zh-TW".to_string(),
        "zh-cn" => "zh-CN".to_string(),
        "en-us" | "en" => "en-US".to_string(),
        "ja-jp" | "ja" => "ja-JP".to_string(),
        "ko-kr" | "ko" => "ko-KR".to_string(),
        "de-de" | "de" => "de-DE".to_string(),
        "fr-fr" | "fr" => "fr-FR".to_string(),
        _ => DEFAULT_LANG.to_string(),
    }
}

fn persist(lang: &str) -> io::Result<()> {
    let path = storage_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, lang)?;
    Ok(())
}

fn storage_path() -> io::Result<PathBuf> {
    let local = dirs::data_local_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "local appdata not found"))?;
    Ok(local.join("Capture2TextPro").join("output_lang.txt"))
}
