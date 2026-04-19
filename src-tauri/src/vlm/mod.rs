use std::io::{self, BufRead, BufReader};
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::{Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use thiserror::Error;

const OLLAMA_CHAT_URL: &str = "http://localhost:11434/api/chat";
const OLLAMA_TAGS_URL: &str = "http://localhost:11434/api/tags";
const OLLAMA_MODEL: &str = "qwen3-vl:8b-instruct";
const VLM_QUEUE_CAPACITY: usize = 4;
const QWEN3VL_MIN_DIM: u32 = 32;
const REQUEST_TIMEOUT_MS: u64 = 30_000;
const HEALTH_TIMEOUT_SECS: u64 = 5;

pub type VlmResult<T> = std::result::Result<T, VlmError>;

#[derive(Debug, Error)]
pub enum VlmError {
    #[error("ollama connection refused (is ollama running?)")]
    OllamaDown,

    #[error("ollama returned HTTP {status}: {body}")]
    OllamaHttpError { status: u16, body: String },

    #[error("vlm request timed out after {}ms", .0)]
    Timeout(u64),

    #[error("image preprocessing failed: {0}")]
    ImagePreprocessing(String),

    #[error(
        "response JSON decode failed: {source_error}; raw={raw_preview}",
        raw_preview = .raw.chars().take(200).collect::<String>()
    )]
    ResponseDecode { raw: String, source_error: String },

    #[error("internal: {0}")]
    Internal(String),
}

impl From<io::Error> for VlmError {
    fn from(err: io::Error) -> Self {
        VlmError::Internal(err.to_string())
    }
}

#[derive(Debug)]
pub enum HealthStatus {
    Healthy,
    OllamaDown,
    ModelMissing { model: String },
    Unknown(String),
}

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

#[derive(Debug, Clone, Serialize)]
pub struct VlmEventPayload {
    pub source: String,
    pub status: String,
    pub original: String,
    pub translated: String,
    pub duration_ms: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VlmPartialEventPayload {
    pub source: String,
    pub original: String,
    pub translated: String,
}

#[derive(Debug, Clone)]
pub struct PartialOutput {
    pub raw_accumulated: String,
    pub original: Option<String>,
    pub translated: Option<String>,
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

pub fn init_worker(app_handle: AppHandle) {
    if VLM_RUNTIME.get().is_some() {
        return;
    }

    let (tx, rx) = sync_channel::<VlmJob>(VLM_QUEUE_CAPACITY);
    let join = match thread::Builder::new()
        .name("vlm-worker".to_string())
        .spawn(move || {
            while let Ok(job) = rx.recv() {
                match ocr_and_translate_streaming(&job.png_bytes, job.target_lang, |partial| {
                    emit_vlm_partial_event(
                        &app_handle,
                        VlmPartialEventPayload {
                            source: job.source.to_string(),
                            original: partial.original.clone().unwrap_or_default(),
                            translated: partial.translated.clone().unwrap_or_default(),
                        },
                    );
                }) {
                    Ok(out) => {
                        println!("[vlm] source={} original: {}", job.source, out.original);
                        println!("[vlm] source={} translated: {}", job.source, out.translated);
                        println!("[vlm] source={} duration_ms: {}", job.source, out.duration_ms);
                        emit_vlm_event(
                            &app_handle,
                            VlmEventPayload {
                                source: job.source.to_string(),
                                status: "success".to_string(),
                                original: out.original,
                                translated: out.translated,
                                duration_ms: out.duration_ms,
                                error: None,
                            },
                        );
                    }
                    Err(err) => {
                        eprintln!("[vlm] source={} failed: {err}", job.source);
                        emit_vlm_event(
                            &app_handle,
                            VlmEventPayload {
                                source: job.source.to_string(),
                                status: "error".to_string(),
                                original: String::new(),
                                translated: String::new(),
                                duration_ms: 0,
                                error: Some(err.to_string()),
                            },
                        );
                    }
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

fn emit_vlm_event(app_handle: &AppHandle, payload: VlmEventPayload) {
    ensure_result_window_visible(app_handle);
    let _ = app_handle.emit_to("result", "vlm-result", &payload);
}

fn emit_vlm_partial_event(app_handle: &AppHandle, payload: VlmPartialEventPayload) {
    ensure_result_window_visible(app_handle);
    let _ = app_handle.emit_to("result", "vlm-result-partial", &payload);
}

fn ensure_result_window_visible(app_handle: &AppHandle) {
    let Some(window) = app_handle.get_webview_window("result") else {
        return;
    };
    if window.is_visible().ok().unwrap_or(false) {
        return;
    }
    let _ = window.center();
    let _ = window.show();
}

pub fn check_health() -> HealthStatus {
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(HEALTH_TIMEOUT_SECS))
        .build()
    {
        Ok(client) => client,
        Err(err) => return HealthStatus::Unknown(format!("client build failed: {err}")),
    };

    let response = match client.get(OLLAMA_TAGS_URL).send() {
        Ok(response) => response,
        Err(err) => {
            if err.is_connect() || err.is_timeout() {
                return HealthStatus::OllamaDown;
            }
            return HealthStatus::Unknown(format!("request failed: {err}"));
        }
    };

    let status = response.status();
    let raw = match response.text() {
        Ok(raw) => raw,
        Err(err) => return HealthStatus::Unknown(format!("read body failed: {err}")),
    };

    if !status.is_success() {
        return HealthStatus::Unknown(format!("HTTP {}: {}", status.as_u16(), raw));
    }

    let tags = match serde_json::from_str::<OllamaTagsResponse>(&raw) {
        Ok(tags) => tags,
        Err(err) => return HealthStatus::Unknown(format!("decode tags failed: {err}")),
    };

    let has_model = tags.models.iter().any(|model| model.name == OLLAMA_MODEL);
    if has_model {
        HealthStatus::Healthy
    } else {
        HealthStatus::ModelMissing {
            model: OLLAMA_MODEL.to_string(),
        }
    }
}

pub fn ocr_and_translate_streaming<F: FnMut(&PartialOutput)>(
    png_bytes: &[u8],
    target_lang: TargetLang,
    mut on_partial: F,
) -> VlmResult<VlmOutput> {
    let png_bytes = ensure_min_dimension(png_bytes)?;
    let started_at = Instant::now();
    let image_b64 = STANDARD.encode(&png_bytes);

    let system_prompt = format!(
        "你是精準的翻譯助理。分析提供的圖片，輸出嚴格 JSON：\n\
{{\"original\":\"<圖片中的完整原文，保留原語言>\",\"translated\":\"<翻譯成 {} 的結果>\"}}\n\
禁止 thinking、禁止解釋、禁止 markdown。",
        target_lang.as_prompt_lang()
    );

    let request = OllamaChatRequest {
        model: OLLAMA_MODEL.to_string(),
        stream: true,
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

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(REQUEST_TIMEOUT_MS))
        .build()
        .map_err(|err| VlmError::Internal(format!("reqwest client build failed: {err}")))?;

    let response = client
        .post(OLLAMA_CHAT_URL)
        .json(&request)
        .send()
        .map_err(map_reqwest_send_error)?;

    let status = response.status();
    if !status.is_success() {
        let raw = response.text().map_err(map_reqwest_send_error)?;
        return Err(VlmError::OllamaHttpError {
            status: status.as_u16(),
            body: raw,
        });
    }

    let mut raw_accumulated = String::new();
    let mut final_duration_ns: Option<u64> = None;
    let reader = BufReader::new(response);
    for line in reader.lines() {
        let line = line.map_err(|err| VlmError::Internal(format!("stream read failed: {err}")))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let chunk = serde_json::from_str::<OllamaChatStreamChunk>(trimmed).map_err(|err| {
            VlmError::ResponseDecode {
                raw: trimmed.to_string(),
                source_error: format!("stream chunk parse failed: {err}"),
            }
        })?;

        if let Some(message) = chunk.message {
            if !message.content.is_empty() {
                raw_accumulated.push_str(&message.content);
                on_partial(&PartialOutput {
                    raw_accumulated: raw_accumulated.clone(),
                    original: extract_partial_json_string(&raw_accumulated, "original"),
                    translated: extract_partial_json_string(&raw_accumulated, "translated"),
                });
            }
        }

        if chunk.done {
            final_duration_ns = chunk.total_duration;
            break;
        }
    }

    let parsed = parse_model_output(&raw_accumulated)?;
    let duration_ms = final_duration_ns
        .map(|ns| ns / 1_000_000)
        .unwrap_or_else(|| started_at.elapsed().as_millis() as u64);

    Ok(VlmOutput {
        original: parsed.original,
        translated: parsed.translated,
        duration_ms,
    })
}

pub fn ocr_and_translate(png_bytes: &[u8], target_lang: TargetLang) -> VlmResult<VlmOutput> {
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

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(REQUEST_TIMEOUT_MS))
        .build()
        .map_err(|err| VlmError::Internal(format!("reqwest client build failed: {err}")))?;

    let response = client
        .post(OLLAMA_CHAT_URL)
        .json(&request)
        .send()
        .map_err(map_reqwest_send_error)?;

    let status = response.status();
    let raw = response.text().map_err(map_reqwest_send_error)?;
    if !status.is_success() {
        return Err(VlmError::OllamaHttpError {
            status: status.as_u16(),
            body: raw,
        });
    }

    let response = serde_json::from_str::<OllamaChatResponse>(&raw).map_err(|err| {
        VlmError::ResponseDecode {
            raw,
            source_error: err.to_string(),
        }
    })?;

    let content = response
        .message
        .as_ref()
        .map(|message| message.content.as_str())
        .ok_or_else(|| VlmError::ResponseDecode {
            raw: "<missing message.content>".to_string(),
            source_error: "missing message.content".to_string(),
        })?;

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

fn ensure_min_dimension(png_bytes: &[u8]) -> VlmResult<Vec<u8>> {
    let img = image::load_from_memory(png_bytes)
        .map_err(|err| VlmError::ImagePreprocessing(format!("decode png failed: {err}")))?;
    let (w, h) = (img.width(), img.height());

    if w == 0 || h == 0 {
        return Err(VlmError::ImagePreprocessing(
            "decode png failed: zero-sized image".to_string(),
        ));
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
        .map_err(|err| VlmError::ImagePreprocessing(format!("encode png failed: {err}")))?;
    Ok(out)
}

fn parse_model_output(content: &str) -> VlmResult<ModelOutput> {
    if let Ok(parsed) = serde_json::from_str::<ModelOutput>(content) {
        return Ok(parsed);
    }

    let json_body = extract_first_json_object(content).ok_or_else(|| VlmError::ResponseDecode {
        raw: content.to_string(),
        source_error: "model content does not contain a JSON object".to_string(),
    })?;

    serde_json::from_str::<ModelOutput>(json_body).map_err(|err| VlmError::ResponseDecode {
        raw: content.to_string(),
        source_error: format!("model JSON parse failed: {err}"),
    })
}

fn map_reqwest_send_error(err: reqwest::Error) -> VlmError {
    if err.is_timeout() {
        VlmError::Timeout(REQUEST_TIMEOUT_MS)
    } else if err.is_connect() {
        VlmError::OllamaDown
    } else {
        VlmError::Internal(format!("ollama request failed: {err}"))
    }
}

fn extract_first_json_object(content: &str) -> Option<&str> {
    let start = content.find('{')?;
    let end = content.rfind('}')?;
    if end < start {
        return None;
    }
    Some(&content[start..=end])
}

fn extract_partial_json_string(raw: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let key_idx = raw.find(&needle)?;
    let after_key = &raw[(key_idx + needle.len())..];
    let colon_idx = after_key.find(':')?;
    let value = after_key[(colon_idx + 1)..].trim_start();
    let value = value.strip_prefix('"')?;

    let mut out = String::new();
    let mut escaped = false;
    for ch in value.chars() {
        if escaped {
            match ch {
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                other => out.push(other),
            }
            escaped = false;
            continue;
        }

        match ch {
            '\\' => escaped = true,
            '"' => return Some(out),
            other => out.push(other),
        }
    }
    Some(out)
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
struct OllamaChatStreamChunk {
    message: Option<OllamaMessageResponse>,
    done: bool,
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

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaTagModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaTagModel {
    name: String,
}
