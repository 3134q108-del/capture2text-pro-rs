pub mod client;
pub mod keyring;

use async_trait::async_trait;

pub use client::AzureProvider;

use crate::tts::TtsError;

#[async_trait]
pub trait TtsProvider: Send + Sync {
    async fn list_voices(&self, lang: &str) -> Result<Vec<Voice>, TtsError>;
    async fn test_connection(&self) -> Result<(), TtsError>;
    #[allow(dead_code)]
    async fn synthesize(
        &self,
        text: &str,
        voice_id: &str,
        rate: f32,
    ) -> Result<Vec<u8>, TtsError>;
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Voice {
    pub id: String,
    pub name: String,
    pub locale: String,
    pub gender: String,
    pub level: VoiceLevel,
    pub sample_rate: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum VoiceLevel {
    Standard,
    HighDefinition,
}
