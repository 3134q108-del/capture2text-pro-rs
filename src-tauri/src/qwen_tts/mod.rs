pub mod downloader;
pub mod runtime;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum VoicePreset {
    Serena,
    Vivian,
    UncleFu,
    Ryan,
    Aiden,
    OnoAnna,
    Sohee,
    Eric,
    Dylan,
}

impl VoicePreset {
    pub fn all() -> [VoicePreset; 9] {
        use VoicePreset::*;
        [Serena, Vivian, UncleFu, Ryan, Aiden, OnoAnna, Sohee, Eric, Dylan]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            VoicePreset::Serena => "Serena",
            VoicePreset::Vivian => "Vivian",
            VoicePreset::UncleFu => "UncleFu",
            VoicePreset::Ryan => "Ryan",
            VoicePreset::Aiden => "Aiden",
            VoicePreset::OnoAnna => "OnoAnna",
            VoicePreset::Sohee => "Sohee",
            VoicePreset::Eric => "Eric",
            VoicePreset::Dylan => "Dylan",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            VoicePreset::Serena => "Serena",
            VoicePreset::Vivian => "Vivian",
            VoicePreset::UncleFu => "UncleFu",
            VoicePreset::Ryan => "Ryan",
            VoicePreset::Aiden => "Aiden",
            VoicePreset::OnoAnna => "OnoAnna",
            VoicePreset::Sohee => "Sohee",
            VoicePreset::Eric => "Eric",
            VoicePreset::Dylan => "Dylan",
        }
    }

    pub fn from_str(s: &str) -> Option<VoicePreset> {
        match s.trim().to_ascii_lowercase().as_str() {
            "serena" => Some(VoicePreset::Serena),
            "vivian" => Some(VoicePreset::Vivian),
            "unclefu" => Some(VoicePreset::UncleFu),
            "ryan" => Some(VoicePreset::Ryan),
            "aiden" => Some(VoicePreset::Aiden),
            "onoanna" => Some(VoicePreset::OnoAnna),
            "sohee" => Some(VoicePreset::Sohee),
            "eric" => Some(VoicePreset::Eric),
            "dylan" => Some(VoicePreset::Dylan),
            _ => None,
        }
    }
}

pub fn bootstrap() -> Result<(), String> {
    downloader::ensure_customvoice_installed()?;
    runtime::init_customvoice()?;
    Ok(())
}

pub fn synthesize(text: &str, preset: VoicePreset, lang: &str) -> Result<Vec<u8>, String> {
    runtime::synthesize_wav(text, preset, lang)
}

pub fn app_tts_dir() -> PathBuf {
    let base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("Capture2TextPro").join("tts_models")
}

pub fn customvoice_model_dir() -> PathBuf {
    app_tts_dir().join("customvoice")
}
