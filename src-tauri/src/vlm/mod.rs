use std::io;
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::{Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};

const OLLAMA_CHAT_URL: &str = "http://localhost:11434/api/chat";
const OLLAMA_MODEL: &str = "qwen3-vl:8b-instruct";
const VLM_QUEUE_CAPACITY: usize = 4;
const QWEN3VL_MIN_DIM: u32 = 32;

static VLM_RUNTIME: OnceLock<VlmRuntime> = OnceLock::new();

struct VlmRuntime {
    tx: SyncSender<VlmJob>,
    _join: Mutex<Option<JoinHandle<()>>>,
}

#[derive(Debug, Clone, Copy)]
pub enum TargetLang {
    Chinese,
    English,
}

pub struct VlmJob {
    pub png_bytes: Vec<u8>,
    pub target_lang: TargetLang,
    pub source: &'static str,
}

impl TargetLang {
    fn as_prompt_lang(self) -> &'static str {
        match self {
            Self::Chinese => "繁體中文",
            Self::English => "English",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct VlmOutput {
    pub original: String,
    pub translated: String,
    pub duration_ms: u64,
}

pub fn init_worker() {
    if VLM_RUNTIME.get().is_some() {
        return;
    }

    let (tx, rx) = sync_channel::<VlmJob>(VLM_QUEUE_CAPACITY);
    let join = match thread::Builder::new()
        .name("vlm-worker".to_string())
        .spawn(move || {
            while let Ok(job) = rx.recv() {
                match ocr_and_translate(&job.png_bytes, job.target_lang) {
                    Ok(out) => {
                        println!("[vlm] source={} original: {}", job.source, out.original);
                        println!("[vlm] source={} translated: {}", job.source, out.translated);
                        println!("[vlm] source={} duration_ms: {}", job.source, out.duration_ms);
                    }
                    Err(err) => eprintln!("[vlm] source={} failed: {err}", job.source),
                }
            }
        }) {
        Ok(handle) => handle,
        Err(err) => {
            eprintln!("[vlm] worker spawn failed: {err}");
            return;
        }
    };

    let _ = VLM_RUNTIME.set(VlmRuntime {
        tx,
        _join: Mutex::new(Some(join)),
    });
}

pub fn try_submit(job: VlmJob) {
    let Some(runtime) = VLM_RUNTIME.get() else {
        eprintln!("[vlm] worker not initialized, dropping request");
        return;
    };

    match runtime.tx.try_send(job) {
        Ok(()) => {}
        Err(TrySendError::Full(job)) => {
            eprintln!("[vlm] queue full, dropping source={}", job.source);
        }
        Err(TrySendError::Disconnected(job)) => {
            eprintln!("[vlm] worker disconnected, dropping source={}", job.source);
        }
    }
}

pub fn ocr_and_translate(png_bytes: &[u8], target_lang: TargetLang) -> io::Result<VlmOutput> {
    let png_bytes = ensure_min_dimension(png_bytes)?;
    let started_at = Instant::now();
    let image_b64 = STANDARD.encode(&png_bytes);

    let system_prompt = format!(
        "你是精準的翻譯助理。分析提供的圖片，輸出嚴格 JSON：\
{{\"original\":\"<圖片中的完整原文，保留原語言>\",\"translated\":\"<翻譯成 {} 的結果>\"}}\
禁止 thinking、禁止解釋、禁止 markdown。",
        target_lang.as_prompt_lang()
    );

    let request = OllamaChatRequest {
        model: OLLAMA_MODEL.to_string(),
        stream: false,
        messages: vec![
            OllamaMessage {
                role: "system".to_string(),
                content: system_prompt,
                images: None,
            },
            OllamaMessage {
                role: "user".to_string(),
                content: "請分析這張圖".to_string(),
                images: Some(vec![image_b64]),
            },
        ],
    };

    let response = reqwest::blocking::Client::new()
        .post(OLLAMA_CHAT_URL)
        .json(&request)
        .send()
        .and_then(|res| res.error_for_status())
        .map_err(|err| io::Error::other(format!("ollama request failed: {err}")))?
        .json::<OllamaChatResponse>()
        .map_err(|err| io::Error::other(format!("ollama response json parse failed: {err}")))?;

    let content = response
        .message
        .as_ref()
        .map(|message| message.content.as_str())
        .ok_or_else(|| io::Error::other("ollama response missing message.content"))?;

    let parsed = parse_model_output(content)?;

    let duration_ms = response
        .total_duration
        .map(|ns| ns / 1_000_000)
        .unwrap_or_else(|| started_at.elapsed().as_millis() as u64);

    Ok(VlmOutput {
        original: parsed.original,
        translated: parsed.translated,
        duration_ms,
    })
}

fn ensure_min_dimension(png_bytes: &[u8]) -> io::Result<Vec<u8>> {
    let img = image::load_from_memory(png_bytes)
        .map_err(|err| io::Error::other(format!("decode png failed: {err}")))?;
    let (w, h) = (img.width(), img.height());

    if w == 0 || h == 0 {
        return Err(io::Error::other("decode png failed: zero-sized image"));
    }
    if w >= QWEN3VL_MIN_DIM && h >= QWEN3VL_MIN_DIM {
        return Ok(png_bytes.to_vec());
    }

    let min_dim = w.min(h);
    let scale = (QWEN3VL_MIN_DIM + min_dim - 1) / min_dim;
    let new_w = w * scale;
    let new_h = h * scale;

    let scaled = img.resize_exact(new_w, new_h, image::imageops::FilterType::Nearest);
    let mut out = Vec::new();
    scaled
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .map_err(|err| io::Error::other(format!("encode png failed: {err}")))?;
    Ok(out)
}

fn parse_model_output(content: &str) -> io::Result<ModelOutput> {
    if let Ok(parsed) = serde_json::from_str::<ModelOutput>(content) {
        return Ok(parsed);
    }

    let json_body = extract_first_json_object(content)
        .ok_or_else(|| io::Error::other("model content does not contain a JSON object"))?;

    serde_json::from_str::<ModelOutput>(json_body)
        .map_err(|err| io::Error::other(format!("model JSON parse failed: {err}")))
}

fn extract_first_json_object(content: &str) -> Option<&str> {
    let start = content.find('{')?;
    let end = content.rfind('}')?;
    if end < start {
        return None;
    }
    Some(&content[start..=end])
}

#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    stream: bool,
    messages: Vec<OllamaMessage>,
}

#[derive(Debug, Serialize)]
struct OllamaMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    images: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    message: Option<OllamaMessageResponse>,
    total_duration: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct OllamaMessageResponse {
    content: String,
}

#[derive(Debug, Deserialize)]
struct ModelOutput {
    original: String,
    translated: String,
}
