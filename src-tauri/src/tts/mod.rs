pub mod config;

use std::io::Cursor;
use std::sync::{Arc, Mutex, OnceLock};

use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::{cpal, Decoder, OutputStream, Sink};

struct PlaybackState {
    sink: Arc<Sink>,
}

static ACTIVE_PLAYBACK: OnceLock<Mutex<Option<PlaybackState>>> = OnceLock::new();

fn playback_slot() -> &'static Mutex<Option<PlaybackState>> {
    ACTIVE_PLAYBACK.get_or_init(|| Mutex::new(None))
}

pub fn init_runtime() -> Result<(), String> {
    crate::qwen_tts::bootstrap()
}

pub fn synthesize_for_active_voice(text: &str, lang: &str) -> Result<Vec<u8>, String> {
    let preset = current_active_preset();
    crate::qwen_tts::synthesize(text, preset, lang)
}

pub fn current_active_preset() -> crate::qwen_tts::VoicePreset {
    let state = crate::window_state::get();
    crate::qwen_tts::VoicePreset::from_str(&state.speech_active_preset)
        .unwrap_or(crate::qwen_tts::VoicePreset::Ryan)
}

pub fn play_wav(bytes: &[u8]) -> Result<(), String> {
    eprintln!("[tts] play_wav called bytes={}", bytes.len());
    if bytes.is_empty() {
        eprintln!("[tts] play_wav: empty audio bytes");
        return Err("empty audio bytes".to_string());
    }
    if bytes.len() >= 44 {
        let riff = &bytes[0..4];
        let wave = &bytes[8..12];
        let fmt = &bytes[12..16];
        let sample_rate = u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]);
        let channels = u16::from_le_bytes([bytes[22], bytes[23]]);
        let bit_depth = u16::from_le_bytes([bytes[34], bytes[35]]);
        let data_size = u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]);
        eprintln!(
            "[tts] WAV header: RIFF={:?} WAVE={:?} fmt_chunk={:?} sr={}Hz ch={} bits={} data_size={} total_bytes={}",
            std::str::from_utf8(riff).ok(),
            std::str::from_utf8(wave).ok(),
            std::str::from_utf8(fmt).ok(),
            sample_rate,
            channels,
            bit_depth,
            data_size,
            bytes.len()
        );

        if bit_depth == 16 && bytes.len() >= 44 + 2 {
            let data_start = 44usize;
            let sample_count = (bytes.len() - data_start) / 2;
            let sample_check = sample_count.min(1000);
            let preview_count = sample_count.min(40);
            let mut max_abs = 0i32;
            let mut sum_abs = 0i64;
            let mut preview = Vec::with_capacity(preview_count);
            for i in 0..sample_check {
                let s = i16::from_le_bytes([bytes[data_start + i * 2], bytes[data_start + i * 2 + 1]]);
                let abs = i32::from(s).abs();
                if abs > max_abs {
                    max_abs = abs;
                }
                sum_abs += i64::from(abs);
                if i < preview_count {
                    preview.push(s);
                }
            }
            let avg_abs = if sample_check > 0 {
                sum_abs / sample_check as i64
            } else {
                0
            };
            eprintln!("[tts] WAV sample preview (first {} i16): {:?}", preview_count, preview);
            eprintln!(
                "[tts] WAV samples (first {}): max_abs={} avg_abs={}",
                sample_check,
                max_abs,
                avg_abs
            );
        }
    }

    eprintln!("[tts] play_wav: stopping current playback if exists");
    stop_current();

    let host = cpal::default_host();
    if let Some(default_dev) = host.default_output_device() {
        let name = default_dev
            .name()
            .unwrap_or_else(|_| "<unknown>".to_string());
        eprintln!("[tts] rodio default output device: {}", name);
    } else {
        eprintln!("[tts] rodio default output device: <none>");
    }

    eprintln!("[tts] play_wav: initializing rodio output");
    let (stream, stream_handle) = OutputStream::try_default().map_err(|e| e.to_string())?;
    let sink = Arc::new(Sink::try_new(&stream_handle).map_err(|e| e.to_string())?);
    let source = Decoder::new(Cursor::new(bytes.to_vec()))
        .map_err(|e| format!("decode wav failed: {e}"))?;
    eprintln!("[tts] play_wav: decoded wav, appending to sink");
    sink.append(source);

    {
        let mut guard = playback_slot()
            .lock()
            .map_err(|_| "playback lock poisoned".to_string())?;
        *guard = Some(PlaybackState {
            sink: Arc::clone(&sink),
        });
    }

    eprintln!("[tts] play_wav: waiting for playback end");
    sink.sleep_until_end();
    eprintln!("[tts] play_wav: playback finished");
    drop(stream);

    if let Ok(mut guard) = playback_slot().lock() {
        let _ = guard.take();
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
