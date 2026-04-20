pub mod config;

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use edge_tts_rust::{EdgeTtsClient, SpeakOptions};
use rodio::{Decoder, OutputStream, Sink};
use thiserror::Error;

pub use config::{TtsConfig, TtsVoiceOption};

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
    #[error("config error: {0}")]
    Config(String),
}

struct PlaybackState {
    sink: Arc<Sink>,
    generation: u64,
}

static ACTIVE_PLAYBACK: OnceLock<Mutex<Option<PlaybackState>>> = OnceLock::new();
static PLAYBACK_GENERATION: AtomicU64 = AtomicU64::new(0);
static TTS_CACHE: OnceLock<Mutex<HashMap<(String, String), Vec<u8>>>> = OnceLock::new();

fn playback_slot() -> &'static Mutex<Option<PlaybackState>> {
    ACTIVE_PLAYBACK.get_or_init(|| Mutex::new(None))
}

fn cache_slot() -> &'static Mutex<HashMap<(String, String), Vec<u8>>> {
    TTS_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn cache_init() {
    let _ = cache_slot();
}

pub fn cache_get(text: &str, voice_code: &str) -> Option<Vec<u8>> {
    let key = (text.to_string(), voice_code.to_string());
    let guard = match cache_slot().lock() {
        Ok(guard) => guard,
        Err(_) => {
            eprintln!("[tts-cache] cache_get lock poisoned");
            return None;
        }
    };
    guard.get(&key).cloned()
}

pub fn cache_put(text: &str, voice_code: &str, mp3: Vec<u8>) {
    let key = (text.to_string(), voice_code.to_string());
    match cache_slot().lock() {
        Ok(mut guard) => {
            guard.insert(key, mp3);
        }
        Err(_) => {
            eprintln!("[tts-cache] cache_put lock poisoned");
        }
    }
}

pub fn prefetch(text: &str, voice_code: &str) {
    if cache_get(text, voice_code).is_some() {
        eprintln!(
            "[tts-cache] prefetch cache hit voice={} text_len={}",
            voice_code,
            text.len()
        );
        return;
    }

    eprintln!(
        "[tts-cache] prefetch cache miss voice={} text_len={}",
        voice_code,
        text.len()
    );

    match synthesize_with_voice(text, voice_code) {
        Ok(mp3) => {
            let size = mp3.len();
            cache_put(text, voice_code, mp3);
            eprintln!(
                "[tts-cache] prefetched {} bytes voice={} text_len={}",
                size,
                voice_code,
                text.len()
            );
        }
        Err(err) => {
            eprintln!(
                "[tts-cache] prefetch failed voice={} text_len={} err={}",
                voice_code,
                text.len(),
                err
            );
        }
    }
}

pub fn init_config_runtime() -> Result<(), TtsError> {
    config::init_runtime().map_err(|err| TtsError::Config(err.to_string()))
}

pub fn available_voices() -> Vec<TtsVoiceOption> {
    config::available_voices()
}

pub fn get_config() -> Result<TtsConfig, TtsError> {
    config::get_config_runtime().map_err(|err| TtsError::Config(err.to_string()))
}

pub fn set_voice(lang: &str, code: String) -> Result<(), TtsError> {
    match lang {
        "zh" => config::set_active_zh(code).map_err(|err| TtsError::Config(err.to_string())),
        "en" => config::set_active_en(code).map_err(|err| TtsError::Config(err.to_string())),
        _ => Err(TtsError::Config("unsupported language".to_string())),
    }
}

pub fn current_voice_for_lang(lang: &str) -> String {
    match lang {
        "en" => config::current_en_voice(),
        _ => config::current_zh_voice(),
    }
}

pub fn preprocess_for_speech(text: &str, voice_code: &str) -> String {
    let is_zh_voice = voice_code.to_ascii_lowercase().starts_with("zh-");
    let replacements = if is_zh_voice {
        [
            (">=", "大於等於"),
            ("<=", "小於等於"),
            ("!=", "不等於"),
            ("==", "等於"),
            (">", "大於"),
            ("<", "小於"),
        ]
    } else {
        [
            (">=", " greater than or equal to "),
            ("<=", " less than or equal to "),
            ("!=", " not equal to "),
            ("==", " equals "),
            (">", " greater than "),
            ("<", " less than "),
        ]
    };

    let mut processed = text.to_string();
    for (pattern, replacement) in replacements {
        processed = processed.replace(pattern, replacement);
    }

    for symbol in ['*', '_', '`', '#', '"'] {
        processed = processed.replace(symbol, "");
    }

    processed
}

pub fn synthesize_with_voice(text: &str, voice_code: &str) -> Result<Vec<u8>, TtsError> {
    if text.trim().is_empty() {
        return Err(TtsError::EmptyText);
    }

    let processed = preprocess_for_speech(text, voice_code);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| TtsError::RuntimeInit(err.to_string()))?;

    runtime.block_on(async move {
        let client = EdgeTtsClient::new().map_err(|err| TtsError::RequestFailed(err.to_string()))?;
        let options = SpeakOptions {
            voice: voice_code.to_string(),
            ..SpeakOptions::default()
        };
        let result = client
            .synthesize(processed.as_str(), options)
            .await
            .map_err(|err| TtsError::RequestFailed(err.to_string()))?;
        Ok(result.audio)
    })
}

pub fn play_mp3(bytes: &[u8]) -> Result<(), TtsError> {
    if bytes.is_empty() {
        return Err(TtsError::PlaybackFailed("empty audio bytes".to_string()));
    }

    stop_current();

    let (stream, stream_handle) =
        OutputStream::try_default().map_err(|err| TtsError::AudioOutput(err.to_string()))?;
    let sink = Arc::new(Sink::try_new(&stream_handle).map_err(|err| {
        TtsError::AudioOutput(format!("create sink failed: {err}"))
    })?);
    let source = Decoder::new(Cursor::new(bytes.to_vec()))
        .map_err(|err| TtsError::PlaybackFailed(format!("decode mp3 failed: {err}")))?;

    sink.append(source);

    let generation = PLAYBACK_GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    {
        let mut guard = playback_slot()
            .lock()
            .map_err(|_| TtsError::PlaybackFailed("playback lock poisoned".to_string()))?;
        *guard = Some(PlaybackState {
            sink: Arc::clone(&sink),
            generation,
        });
    }

    eprintln!(
        "[tts] play_mp3 started bytes={} generation={}",
        bytes.len(),
        generation
    );
    sink.sleep_until_end();
    eprintln!("[tts] play_mp3 finished generation={}", generation);
    drop(stream);

    if let Ok(mut guard) = playback_slot().lock() {
        if guard
            .as_ref()
            .map(|state| state.generation == generation)
            .unwrap_or(false)
        {
            let _ = guard.take();
        }
    }

    Ok(())
}

pub fn stop_current() {
    if let Ok(mut guard) = playback_slot().lock() {
        if let Some(state) = guard.take() {
            state.sink.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::preprocess_for_speech;

    #[test]
    fn preprocess_replaces_operators_for_en_voice() {
        let text = r#"a >= b <= c != d == e > f < g * _ ` # " ~"#;
        let processed = preprocess_for_speech(text, "en-US-AndrewNeural");

        assert!(processed.contains("greater than or equal to"));
        assert!(processed.contains("less than or equal to"));
        assert!(processed.contains("not equal to"));
        assert!(processed.contains("equals"));
        assert!(processed.contains("greater than"));
        assert!(processed.contains("less than"));
        assert!(!processed.contains(">="));
        assert!(!processed.contains("<="));
        assert!(!processed.contains("!="));
        assert!(!processed.contains("=="));
        assert!(!processed.contains('*'));
        assert!(!processed.contains('_'));
        assert!(!processed.contains('`'));
        assert!(!processed.contains('#'));
        assert!(!processed.contains('"'));
        assert!(processed.contains('~'));
    }

    #[test]
    fn preprocess_replaces_operators_for_zh_voice() {
        let text = "a>=b<=c!=d==e>f<g~";
        let processed = preprocess_for_speech(text, "zh-TW-HsiaoChenNeural");

        assert!(processed.contains("大於等於"));
        assert!(processed.contains("小於等於"));
        assert!(processed.contains("不等於"));
        assert!(processed.contains("等於"));
        assert!(processed.contains("大於"));
        assert!(processed.contains("小於"));
        assert!(processed.contains('~'));
    }
}
