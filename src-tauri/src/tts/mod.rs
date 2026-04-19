use std::io::Cursor;
use std::thread;

use edge_tts_rust::{EdgeTtsClient, SpeakOptions};
use rodio::{Decoder, OutputStream, Sink};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TtsError {
    #[error("empty text")]
    EmptyText,
    #[error("tts runtime init failed: {0}")]
    RuntimeInit(String),
    #[error("tts request failed: {0}")]
    RequestFailed(String),
    #[error("audio output unavailable: {0}")]
    AudioOutput(String),
    #[error("audio playback failed: {0}")]
    PlaybackFailed(String),
}

#[derive(Debug, Clone, Copy)]
pub enum TtsVoice {
    Chinese,
    English,
}

impl TtsVoice {
    fn as_name(self) -> &'static str {
        match self {
            Self::Chinese => "zh-TW-HsiaoChenNeural",
            Self::English => "en-US-AvaNeural",
        }
    }
}

pub fn synthesize(text: &str, voice: TtsVoice) -> Result<Vec<u8>, TtsError> {
    if text.trim().is_empty() {
        return Err(TtsError::EmptyText);
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| TtsError::RuntimeInit(err.to_string()))?;

    runtime.block_on(async move {
        let client = EdgeTtsClient::new().map_err(|err| TtsError::RequestFailed(err.to_string()))?;
        let options = SpeakOptions {
            voice: voice.as_name().to_string(),
            ..SpeakOptions::default()
        };
        let result = client
            .synthesize(text, options)
            .await
            .map_err(|err| TtsError::RequestFailed(err.to_string()))?;
        Ok(result.audio)
    })
}

pub fn play_mp3(bytes: &[u8]) -> Result<(), TtsError> {
    if bytes.is_empty() {
        return Err(TtsError::PlaybackFailed("empty audio bytes".to_string()));
    }

    let audio = bytes.to_vec();
    thread::Builder::new()
        .name("tts-playback".to_string())
        .spawn(move || {
            let Ok((_stream, stream_handle)) = OutputStream::try_default() else {
                eprintln!("[tts] audio output unavailable");
                return;
            };
            let Ok(sink) = Sink::try_new(&stream_handle) else {
                eprintln!("[tts] sink init failed");
                return;
            };
            let cursor = Cursor::new(audio);
            let Ok(source) = Decoder::new(cursor) else {
                eprintln!("[tts] decoder init failed");
                return;
            };
            sink.append(source);
            sink.sleep_until_end();
        })
        .map_err(|err| TtsError::AudioOutput(err.to_string()))?;

    Ok(())
}
