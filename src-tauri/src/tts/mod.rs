use thiserror::Error;

#[derive(Debug, Error, serde::Serialize)]
#[allow(dead_code)]
pub enum TtsError {
    #[error("Azure TTS not configured (set API key in Settings > Speech)")]
    NotConfigured,

    #[error("Azure TTS not yet implemented")]
    NotImplemented,

    #[error("Network error: {0}")]
    Network(String),

    #[error("Authentication failed (check API key)")]
    Auth,

    #[error("Region invalid: {0}")]
    BadRegion(String),

    #[error("Voice not found: {0}")]
    VoiceNotFound(String),

    #[error("Azure API returned {status}: {message}")]
    Api { status: u16, message: String },

    #[error("Keyring error: {0}")]
    Keyring(String),
}

// T52 placeholder: Task 2 will replace this with AzureProvider and TtsProvider.
#[allow(dead_code)]
pub fn synth_bytes(_text: &str, _voice: &str, _lang: &str) -> Result<Vec<u8>, TtsError> {
    Err(TtsError::NotImplemented)
}
