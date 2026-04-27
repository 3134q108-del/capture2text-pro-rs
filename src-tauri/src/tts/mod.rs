use thiserror::Error;

#[derive(Debug, Error, serde::Serialize)]
#[allow(dead_code)]
pub enum TtsError {
    #[error("Azure TTS not configured (set API key in Settings > Speech)")]
    NotConfigured,
    #[error("Azure TTS not yet implemented (T52 in progress)")]
    NotImplemented,
}

// T52 placeholder: Task 2 will replace this with AzureProvider and TtsProvider.
#[allow(dead_code)]
pub fn synth_bytes(_text: &str, _voice: &str, _lang: &str) -> Result<Vec<u8>, TtsError> {
    Err(TtsError::NotImplemented)
}
