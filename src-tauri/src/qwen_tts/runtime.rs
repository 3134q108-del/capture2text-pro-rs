use std::io::Cursor;
use std::sync::{Mutex, OnceLock};

use qwen3_tts::{
    auto_device, compute_dtype_for_device, AudioBuffer, Language, Qwen3TTS, Speaker,
    SynthesisOptions,
};

static MODEL: OnceLock<Mutex<Option<Qwen3TTS>>> = OnceLock::new();

pub fn init_customvoice() -> Result<(), String> {
    let model_dir = super::customvoice_model_dir();
    if !model_dir.exists() {
        return Err(format!(
            "customvoice model directory not found: {}",
            model_dir.display()
        ));
    }

    let device = auto_device().map_err(|e| e.to_string())?;
    let compute_dtype = compute_dtype_for_device(&device);
    eprintln!(
        "[qwen-tts] init device={} compute_dtype={compute_dtype:?}",
        qwen3_tts::device_info(&device)
    );
    let model = Qwen3TTS::from_pretrained(model_dir.to_string_lossy().as_ref(), device)
        .map_err(|e| e.to_string())?;

    let slot = MODEL.get_or_init(|| Mutex::new(None));
    let mut guard = slot.lock().map_err(|_| "model lock poisoned".to_string())?;
    *guard = Some(model);
    eprintln!("[qwen-tts] CustomVoice loaded from {}", model_dir.display());
    Ok(())
}

pub fn synthesize_wav(
    text: &str,
    preset: super::VoicePreset,
    lang_hint: &str,
) -> Result<Vec<u8>, String> {
    if text.trim().is_empty() {
        return Err("empty text".to_string());
    }

    let slot = MODEL
        .get()
        .ok_or_else(|| "qwen model not initialized".to_string())?;
    let guard = slot.lock().map_err(|_| "model lock poisoned".to_string())?;
    let model = guard
        .as_ref()
        .ok_or_else(|| "qwen model not loaded".to_string())?;

    let speaker = map_preset_to_speaker(preset);
    let language = map_lang(lang_hint);
    let options = SynthesisOptions {
        temperature: 0.9,
        top_k: 30,
        max_length: 2048,
        chunk_frames: 10,
        ..Default::default()
    };
    let stream = model
        .synthesize_streaming(text, speaker, language, options)
        .map_err(|e| e.to_string())?;
    let mut all_samples = Vec::new();
    let mut sample_rate = 24_000u32;
    let mut chunk_count = 0usize;
    for chunk_result in stream {
        let chunk = chunk_result.map_err(|e| e.to_string())?;
        sample_rate = chunk.sample_rate;
        all_samples.extend_from_slice(&chunk.samples);
        chunk_count += 1;
    }
    if all_samples.is_empty() {
        return Err("streaming synthesis returned empty audio".to_string());
    }
    eprintln!(
        "[qwen-tts] synthesize_streaming done chunks={} samples={} sr={}Hz",
        chunk_count,
        all_samples.len(),
        sample_rate
    );
    let audio = AudioBuffer::new(all_samples, sample_rate);

    audio_to_wav_bytes(&audio).map_err(|e| e.to_string())
}

fn map_preset_to_speaker(preset: super::VoicePreset) -> Speaker {
    match preset {
        super::VoicePreset::Serena => Speaker::Serena,
        super::VoicePreset::Vivian => Speaker::Vivian,
        super::VoicePreset::UncleFu => Speaker::UncleFu,
        super::VoicePreset::Ryan => Speaker::Ryan,
        super::VoicePreset::Aiden => Speaker::Aiden,
        super::VoicePreset::OnoAnna => Speaker::OnoAnna,
        super::VoicePreset::Sohee => Speaker::Sohee,
        super::VoicePreset::Eric => Speaker::Eric,
        super::VoicePreset::Dylan => Speaker::Dylan,
    }
}

fn map_lang(lang_hint: &str) -> Language {
    match lang_hint {
        "zh-TW" | "zh-CN" | "zh" => Language::Chinese,
        "ja-JP" | "ja" => Language::Japanese,
        "ko-KR" | "ko" => Language::Korean,
        "de-DE" | "de" => Language::German,
        "fr-FR" | "fr" => Language::French,
        _ => Language::English,
    }
}

fn audio_to_wav_bytes(audio: &AudioBuffer) -> Result<Vec<u8>, hound::Error> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: audio.sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut buffer = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut buffer, spec)?;
        for &sample in &audio.samples {
            let v = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
            writer.write_sample(v)?;
        }
        writer.finalize()?;
    }
    Ok(buffer.into_inner())
}
