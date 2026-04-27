use serde::Serialize;
use std::collections::HashMap;

use crate::azure_tts::{AzureProvider, TtsProvider, Voice};

#[derive(Debug, Clone, Serialize)]
pub struct AzureCredentialsStatus {
    pub configured: bool,
    pub region: Option<String>,
}

#[tauri::command]
pub async fn save_azure_credentials(key: String, region: String) -> Result<(), String> {
    let key = key.trim();
    let region = normalize_region(&region)?;
    if key.is_empty() {
        return Err("Azure subscription key is required".to_string());
    }
    crate::azure_tts::keyring::save_key(key).map_err(|err| err.to_string())?;
    crate::window_state::set_azure_region(Some(region));
    Ok(())
}

#[tauri::command]
pub fn get_azure_credentials_status() -> AzureCredentialsStatus {
    let configured = match crate::azure_tts::keyring::has_key() {
        Ok(value) => value,
        Err(err) => {
            eprintln!("[azure-tts] keyring status failed: {err}");
            false
        }
    };
    AzureCredentialsStatus {
        configured,
        region: crate::window_state::azure_region(),
    }
}

#[tauri::command]
pub fn delete_azure_credentials() -> Result<(), String> {
    crate::azure_tts::keyring::delete_key().map_err(|err| err.to_string())?;
    crate::window_state::set_azure_region(None);
    Ok(())
}

#[tauri::command]
pub async fn test_azure_connection() -> Result<(), String> {
    provider_from_config()?
        .test_connection()
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn list_azure_voices(lang: String) -> Result<Vec<Voice>, String> {
    provider_from_config()?
        .list_voices(&lang)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_voice_routing() -> HashMap<String, String> {
    crate::window_state::azure_voice_map()
}

#[tauri::command]
pub fn set_voice_routing(lang: String, voice_id: String) -> Result<(), String> {
    let lang = lang.trim();
    let voice_id = voice_id.trim();
    if lang.is_empty() {
        return Err("language is required".to_string());
    }
    if voice_id.is_empty() {
        crate::window_state::clear_azure_voice_for_lang(lang);
    } else {
        crate::window_state::set_azure_voice_for_lang(lang.to_string(), voice_id.to_string());
    }
    Ok(())
}

fn provider_from_config() -> Result<AzureProvider, String> {
    let region = crate::window_state::azure_region()
        .ok_or_else(|| crate::tts::TtsError::NotConfigured.to_string())?;
    let key = crate::azure_tts::keyring::get_key()
        .map_err(|err| err.to_string())?
        .ok_or_else(|| crate::tts::TtsError::NotConfigured.to_string())?;
    Ok(AzureProvider::new(region, key))
}

fn normalize_region(region: &str) -> Result<String, String> {
    let normalized = region.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        Err("Azure region is required".to_string())
    } else {
        Ok(normalized)
    }
}
