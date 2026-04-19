use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsConfig {
    pub active_zh: String,
    pub active_en: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsVoiceOption {
    pub code: String,
    pub display_name: String,
    pub lang: String,
}

#[derive(Debug)]
struct TtsRuntime {
    path: PathBuf,
    config: TtsConfig,
}

static TTS_RUNTIME: OnceLock<Mutex<TtsRuntime>> = OnceLock::new();

pub fn available_voices() -> Vec<TtsVoiceOption> {
    vec![
        TtsVoiceOption {
            code: "zh-TW-HsiaoChenNeural".to_string(),
            display_name: "HsiaoChen (Female)".to_string(),
            lang: "zh".to_string(),
        },
        TtsVoiceOption {
            code: "zh-TW-HsiaoYuNeural".to_string(),
            display_name: "HsiaoYu (Female)".to_string(),
            lang: "zh".to_string(),
        },
        TtsVoiceOption {
            code: "zh-TW-YunJheNeural".to_string(),
            display_name: "YunJhe (Male)".to_string(),
            lang: "zh".to_string(),
        },
        TtsVoiceOption {
            code: "en-US-AvaNeural".to_string(),
            display_name: "Ava (Female)".to_string(),
            lang: "en".to_string(),
        },
        TtsVoiceOption {
            code: "en-US-AndrewNeural".to_string(),
            display_name: "Andrew (Male)".to_string(),
            lang: "en".to_string(),
        },
        TtsVoiceOption {
            code: "en-US-EmmaNeural".to_string(),
            display_name: "Emma (Female)".to_string(),
            lang: "en".to_string(),
        },
    ]
}

pub fn default_config() -> TtsConfig {
    TtsConfig {
        active_zh: "zh-TW-HsiaoChenNeural".to_string(),
        active_en: "en-US-AvaNeural".to_string(),
    }
}

pub fn storage_path() -> io::Result<PathBuf> {
    let local = dirs::data_local_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "local appdata not found"))?;
    Ok(local.join("Capture2TextPro").join("tts_config.json"))
}

pub fn load_config() -> io::Result<TtsConfig> {
    let path = storage_path()?;
    if !path.exists() {
        return Ok(default_config());
    }
    let raw = fs::read_to_string(path)?;
    let parsed = serde_json::from_str::<TtsConfig>(&raw)
        .map_err(|err| io::Error::other(format!("parse tts_config.json failed: {err}")))?;
    Ok(sanitize_config(parsed))
}

pub fn save_config(config: &TtsConfig) -> io::Result<()> {
    let path = storage_path()?;
    write_config(&path, &sanitize_config(config.clone()))
}

pub fn init_runtime() -> io::Result<()> {
    if TTS_RUNTIME.get().is_some() {
        return Ok(());
    }

    let path = storage_path()?;
    let config = if path.exists() {
        load_config()?
    } else {
        default_config()
    };
    write_config(&path, &config)?;

    let _ = TTS_RUNTIME.set(Mutex::new(TtsRuntime { path, config }));
    Ok(())
}

pub fn get_config_runtime() -> io::Result<TtsConfig> {
    let runtime = runtime_guard()?;
    Ok(runtime.config.clone())
}

pub fn current_zh_voice() -> String {
    match runtime_guard() {
        Ok(runtime) => runtime.config.active_zh.clone(),
        Err(_) => default_config().active_zh,
    }
}

pub fn current_en_voice() -> String {
    match runtime_guard() {
        Ok(runtime) => runtime.config.active_en.clone(),
        Err(_) => default_config().active_en,
    }
}

pub fn set_active_zh(code: String) -> io::Result<()> {
    set_active("zh", code)
}

pub fn set_active_en(code: String) -> io::Result<()> {
    set_active("en", code)
}

pub fn set_active(lang: &str, code: String) -> io::Result<()> {
    let voices = available_voices();
    let exists = voices.iter().any(|voice| voice.lang == lang && voice.code == code);
    if !exists {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "voice code is not valid for language",
        ));
    }

    let mut runtime = runtime_guard()?;
    match lang {
        "zh" => runtime.config.active_zh = code,
        "en" => runtime.config.active_en = code,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unsupported language",
            ))
        }
    }
    write_config(&runtime.path, &runtime.config)
}

fn sanitize_config(config: TtsConfig) -> TtsConfig {
    let voices = available_voices();
    let mut next = config;
    if !voices
        .iter()
        .any(|voice| voice.lang == "zh" && voice.code == next.active_zh)
    {
        next.active_zh = default_config().active_zh;
    }
    if !voices
        .iter()
        .any(|voice| voice.lang == "en" && voice.code == next.active_en)
    {
        next.active_en = default_config().active_en;
    }
    next
}

fn write_config(path: &PathBuf, config: &TtsConfig) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(config)
        .map_err(|err| io::Error::other(format!("serialize tts config failed: {err}")))?;
    fs::write(path, raw)?;
    Ok(())
}

fn runtime_guard() -> io::Result<std::sync::MutexGuard<'static, TtsRuntime>> {
    let runtime = TTS_RUNTIME
        .get()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "tts runtime not initialized"))?;
    runtime
        .lock()
        .map_err(|_| io::Error::other("tts runtime lock poisoned"))
}
