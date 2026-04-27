use std::io::{self, BufRead, BufReader};
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::{Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use thiserror::Error;

use crate::{llama_runtime, scenarios};

pub mod state;

const LLAMA_CHAT_URL: &str = "http://127.0.0.1:11434/v1/chat/completions";
const CHAT_MODEL_NAME: &str = "local";
const VLM_QUEUE_CAPACITY: usize = 4;
const QWEN3VL_MIN_DIM: u32 = 32;
const REQUEST_TIMEOUT_MS: u64 = 90_000;

pub type VlmResult<T> = std::result::Result<T, VlmError>;

#[derive(Debug, Error)]
pub enum VlmError {
    #[error("llama-server connection refused (is runtime running?)")]
    OllamaDown,
    #[error("llama-server returned HTTP {status}: {body}")]
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

#[derive(Debug, Clone, Serialize)]
pub struct HealthWarning {
    pub status: String,
    pub message: String,
}

impl HealthStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::OllamaDown => "ollama-down",
            Self::ModelMissing { .. } => "model-missing",
            Self::Unknown(_) => "unknown",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::Healthy => "OK".to_string(),
            Self::OllamaDown => "llama.cpp runtime is not ready".to_string(),
            Self::ModelMissing { model } => format!("model missing: {model}"),
            Self::Unknown(msg) => msg.clone(),
        }
    }

    pub fn to_warning(&self) -> Option<HealthWarning> {
        if matches!(self, Self::Healthy) {
            None
        } else {
            Some(HealthWarning {
                status: self.label().to_string(),
                message: self.message(),
            })
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetLang {
    TraditionalChinese,
    SimplifiedChinese,
    English,
    Japanese,
    Korean,
    German,
    French,
}

impl TargetLang {
    fn display_name(self) -> &'static str {
        match self {
            Self::TraditionalChinese => "繁體中文",
            Self::SimplifiedChinese => "簡體中文",
            Self::English => "英文",
            Self::Japanese => "日文",
            Self::Korean => "韓文",
            Self::German => "德文",
            Self::French => "法文",
        }
    }
}

pub enum VlmJob {
    OcrAndTranslate {
        png_bytes: Vec<u8>,
        target_lang: TargetLang,
        source: &'static str,
    },
    TranslateText {
        text: String,
        target_lang: TargetLang,
        source: &'static str,
    },
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

#[derive(Debug, Serialize)]
pub struct VlmOutput {
    pub original: String,
    pub translated: String,
    pub duration_ms: u64,
}

static VLM_RUNTIME: OnceLock<VlmRuntime> = OnceLock::new();

struct VlmRuntime {
    tx: SyncSender<VlmJob>,
    _join: Mutex<Option<JoinHandle<()>>>,
}

pub fn init_worker(app_handle: AppHandle) {
    if VLM_RUNTIME.get().is_some() {
        return;
    }
    state::init();

    let (tx, rx) = sync_channel::<VlmJob>(VLM_QUEUE_CAPACITY);
    let join = match thread::Builder::new()
        .name("vlm-worker".to_string())
        .spawn(move || {
            while let Ok(job) = rx.recv() {
                state::set_loading(job_source(&job));
                let source_label = job_source(&job).to_string();
                let lang_code = target_lang_to_code(job_target_lang(&job));
                if let Err(err) = llama_runtime::ensure_model_for_lang(lang_code) {
                    let message = format!(
                        "switch model for lang {lang_code} failed: {err}"
                    );
                    eprintln!("[vlm] source={} failed: {}", source_label, message);
                    emit_vlm_event(
                        &app_handle,
                        VlmEventPayload {
                            source: source_label,
                            status: "error".to_string(),
                            original: String::new(),
                            translated: String::new(),
                            duration_ms: 0,
                            error: Some(message),
                        },
                    );
                    continue;
                }

                let (source, result) = match job {
                    VlmJob::OcrAndTranslate {
                        png_bytes,
                        target_lang,
                        source,
                    } => {
                        let source_label = source.to_string();
                        let source_for_partial = source_label.clone();
                        let result = ocr_and_translate_streaming(&png_bytes, target_lang, |partial| {
                            emit_vlm_partial_event(
                                &app_handle,
                                VlmPartialEventPayload {
                                    source: source_for_partial.clone(),
                                    original: partial.original.clone().unwrap_or_default(),
                                    translated: partial.translated.clone().unwrap_or_default(),
                                },
                            );
                        });
                        (source_label, result)
                    }
                    VlmJob::TranslateText {
                        text,
                        target_lang,
                        source,
                    } => {
                        let source_label = source.to_string();
                        let source_for_partial = source_label.clone();
                        let result = translate_text_streaming(&text, target_lang, |partial| {
                            emit_vlm_partial_event(
                                &app_handle,
                                VlmPartialEventPayload {
                                    source: source_for_partial.clone(),
                                    original: partial.original.clone().unwrap_or_default(),
                                    translated: partial.translated.clone().unwrap_or_default(),
                                },
                            );
                        });
                        (source_label, result)
                    }
                };

                match result {
                    Ok(out) => {
                        println!("[vlm] source={} original: {}", source, out.original);
                        println!("[vlm] source={} translated: {}", source, out.translated);
                        println!("[vlm] source={} duration_ms: {}", source, out.duration_ms);
                        emit_vlm_event(
                            &app_handle,
                            VlmEventPayload {
                                source,
                                status: "success".to_string(),
                                original: out.original,
                                translated: out.translated,
                                duration_ms: out.duration_ms,
                                error: None,
                            },
                        );
                    }
                    Err(err) => {
                        eprintln!("[vlm] source={} failed: {err}", source);
                        emit_vlm_event(
                            &app_handle,
                            VlmEventPayload {
                                source,
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

pub fn try_submit_ocr(png_bytes: Vec<u8>, target_lang: TargetLang, source: &'static str) {
    try_submit(VlmJob::OcrAndTranslate {
        png_bytes,
        target_lang,
        source,
    });
}

pub fn try_submit_text(text: String, target_lang: TargetLang, source: &'static str) {
    try_submit(VlmJob::TranslateText {
        text,
        target_lang,
        source,
    });
}

fn try_submit(job: VlmJob) {
    let Some(runtime) = VLM_RUNTIME.get() else {
        eprintln!("[vlm] worker not initialized, dropping request");
        return;
    };

    match runtime.tx.try_send(job) {
        Ok(()) => {}
        Err(TrySendError::Full(job)) => {
            eprintln!("[vlm] queue full, dropping source={}", job_source(&job));
        }
        Err(TrySendError::Disconnected(job)) => {
            eprintln!("[vlm] worker disconnected, dropping source={}", job_source(&job));
        }
    }
}

fn job_source(job: &VlmJob) -> &'static str {
    match job {
        VlmJob::OcrAndTranslate { source, .. } => source,
        VlmJob::TranslateText { source, .. } => source,
    }
}

fn job_target_lang(job: &VlmJob) -> TargetLang {
    match job {
        VlmJob::OcrAndTranslate { target_lang, .. } => *target_lang,
        VlmJob::TranslateText { target_lang, .. } => *target_lang,
    }
}

fn target_lang_to_code(lang: TargetLang) -> &'static str {
    match lang {
        TargetLang::TraditionalChinese => "zh-TW",
        TargetLang::SimplifiedChinese => "zh-CN",
        TargetLang::English => "en-US",
        TargetLang::Japanese => "ja-JP",
        TargetLang::Korean => "ko-KR",
        TargetLang::German => "de-DE",
        TargetLang::French => "fr-FR",
    }
}

fn emit_vlm_event(app_handle: &AppHandle, payload: VlmEventPayload) {
    eprintln!(
        "[emit] vlm-result status={} source={} original.len={} translated.len={}",
        payload.status,
        payload.source,
        payload.original.len(),
        payload.translated.len()
    );

    if payload.status == "success" {
        crate::capture::log::append_capture(&payload.original, &payload.translated);
        crate::clipboard::write_capture(&payload.original, &payload.translated);
    }

    match payload.status.as_str() {
        "success" => state::set_success(
            &payload.source,
            &payload.original,
            &payload.translated,
            payload.duration_ms,
        ),
        "error" => state::set_error(
            &payload.source,
            payload.error.as_deref().unwrap_or("unknown error"),
        ),
        _ => {}
    }
    ensure_result_window_visible(app_handle);
    let _ = app_handle.emit_to("result", "vlm-result", &payload);
}

fn emit_vlm_partial_event(app_handle: &AppHandle, payload: VlmPartialEventPayload) {
    eprintln!(
        "[emit] vlm-result-partial source={} original.len={} translated.len={}",
        payload.source,
        payload.original.len(),
        payload.translated.len()
    );
    state::set_partial(&payload.source, &payload.original, &payload.translated);
    ensure_result_window_visible(app_handle);
    let _ = app_handle.emit_to("result", "vlm-result-partial", &payload);
}

fn ensure_result_window_visible(app_handle: &AppHandle) {
    let window = match crate::commands::result_window::ensure_webview_window(
        app_handle.clone(),
        "result",
    ) {
        Ok(window) => window,
        Err(err) => {
            eprintln!(
                "[emit] ensure_result_window_visible: window_exists={} was_visible={} err={}",
                false, false, err
            );
            return;
        }
    };
    let was_visible = window.is_visible().ok().unwrap_or(false);
    eprintln!(
        "[emit] ensure_result_window_visible: window_exists={} was_visible={}",
        true, was_visible
    );
    if was_visible {
        return;
    }
    if let Err(err) = window.show() {
        eprintln!(
            "[emit] ensure_result_window_visible: show failed err={}",
            err
        );
        return;
    }
    thread::sleep(Duration::from_millis(50));
}

pub fn check_health() -> HealthStatus {
    if llama_runtime::supervisor::is_healthy() {
        HealthStatus::Healthy
    } else {
        HealthStatus::OllamaDown
    }
}

pub fn warmup() {
    thread::spawn(|| {
        eprintln!("[vlm-warmup] start");
        let t0 = Instant::now();
        let client = match reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(180_000))
            .build()
        {
            Ok(client) => client,
            Err(err) => {
                eprintln!("[vlm-warmup] client build failed: {err}");
                return;
            }
        };

        let request = ChatRequest {
            model: CHAT_MODEL_NAME.to_string(),
            stream: false,
            messages: vec![ChatMessage::new_text("user", "hi".to_string())],
        };

        match client.post(LLAMA_CHAT_URL).json(&request).send() {
            Ok(resp) => {
                let ok = resp.status().is_success();
                eprintln!(
                    "[vlm-warmup] done in {}ms status={} ok={}",
                    t0.elapsed().as_millis(),
                    resp.status(),
                    ok
                );
            }
            Err(err) => {
                eprintln!("[vlm-warmup] failed: {err}");
            }
        }
    });
}

pub fn ocr_and_translate_streaming<F: FnMut(&PartialOutput)>(
    png_bytes: &[u8],
    target_lang: TargetLang,
    mut on_partial: F,
) -> VlmResult<VlmOutput> {
    let png_bytes = ensure_min_dimension(png_bytes)?;
    let started_at = Instant::now();
    let image_b64 = STANDARD.encode(&png_bytes);
    let request = build_chat_request(
        build_system_prompt(target_lang),
        "請分析這張圖片中的文字並翻譯。".to_string(),
        Some(vec![image_b64]),
        true,
    );

    let (raw_accumulated, duration_ns) = run_streaming_request(request, |raw| {
        on_partial(&PartialOutput {
            raw_accumulated: raw.to_string(),
            original: extract_partial_json_string(raw, "original"),
            translated: extract_partial_json_string(raw, "translated"),
        });
    })?;

    let parsed = parse_model_output(&raw_accumulated)?;
    let duration_ms = duration_ns
        .map(|ns| ns / 1_000_000)
        .unwrap_or_else(|| started_at.elapsed().as_millis() as u64);
    Ok(VlmOutput {
        original: parsed.original,
        translated: parsed.translated,
        duration_ms,
    })
}

pub fn translate_text_streaming<F: FnMut(&PartialOutput)>(
    text: &str,
    target_lang: TargetLang,
    mut on_partial: F,
) -> VlmResult<VlmOutput> {
    let started_at = Instant::now();
    let request = build_chat_request(
        build_system_prompt(target_lang),
        text.to_string(),
        None,
        true,
    );

    let (raw_accumulated, duration_ns) = run_streaming_request(request, |raw| {
        on_partial(&PartialOutput {
            raw_accumulated: raw.to_string(),
            original: Some(text.to_string()),
            translated: extract_partial_json_string(raw, "translated"),
        });
    })?;

    let parsed = parse_model_output(&raw_accumulated)?;
    let duration_ms = duration_ns
        .map(|ns| ns / 1_000_000)
        .unwrap_or_else(|| started_at.elapsed().as_millis() as u64);
    Ok(VlmOutput {
        original: text.to_string(),
        translated: parsed.translated,
        duration_ms,
    })
}

pub fn ocr_and_translate(png_bytes: &[u8], target_lang: TargetLang) -> VlmResult<VlmOutput> {
    let png_bytes = ensure_min_dimension(png_bytes)?;
    let started_at = Instant::now();
    let image_b64 = STANDARD.encode(&png_bytes);
    let request = build_chat_request(
        build_system_prompt(target_lang),
        "請分析這張圖片中的文字並翻譯。".to_string(),
        Some(vec![image_b64]),
        false,
    );

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(REQUEST_TIMEOUT_MS))
        .build()
        .map_err(|err| VlmError::Internal(format!("reqwest client build failed: {err}")))?;

    let response = client
        .post(LLAMA_CHAT_URL)
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

    let response = serde_json::from_str::<ChatResponse>(&raw).map_err(|err| {
        VlmError::ResponseDecode {
            raw,
            source_error: err.to_string(),
        }
    })?;

    let content = response
        .choices
        .first()
        .and_then(|choice| choice.message.as_ref())
        .and_then(|msg| msg.content.as_deref())
        .ok_or_else(|| VlmError::ResponseDecode {
            raw: "<missing choices[0].message.content>".to_string(),
            source_error: "missing choices[0].message.content".to_string(),
        })?;

    let parsed = parse_model_output(content)?;
    let duration_ms = started_at.elapsed().as_millis() as u64;
    Ok(VlmOutput {
        original: parsed.original,
        translated: parsed.translated,
        duration_ms,
    })
}

fn run_streaming_request<F: FnMut(&str)>(
    request: ChatRequest,
    mut on_partial_raw: F,
) -> VlmResult<(String, Option<u64>)> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(REQUEST_TIMEOUT_MS))
        .build()
        .map_err(|err| VlmError::Internal(format!("reqwest client build failed: {err}")))?;

    let response = client
        .post(LLAMA_CHAT_URL)
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
    let final_duration_ns: Option<u64> = None;
    let reader = BufReader::new(response);
    for line in reader.lines() {
        let line = line.map_err(|err| VlmError::Internal(format!("stream read failed: {err}")))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(payload) = trimmed.strip_prefix("data:") {
            let payload = payload.trim();
            if payload == "[DONE]" {
                break;
            }

            let chunk = serde_json::from_str::<ChatStreamChunk>(payload).map_err(|err| {
                VlmError::ResponseDecode {
                    raw: payload.to_string(),
                    source_error: format!("stream chunk parse failed: {err}"),
                }
            })?;

            if let Some(choice) = chunk.choices.first() {
                if let Some(content) = choice.delta.content.as_deref() {
                    if !content.is_empty() {
                        raw_accumulated.push_str(content);
                        on_partial_raw(&raw_accumulated);
                    }
                }
            }
        }
    }

    Ok((raw_accumulated, final_duration_ns))
}

fn build_system_prompt(target_lang: TargetLang) -> String {
    let scenario = scenarios::current_scenario();
    format!(
        "{}\n\n輸出嚴格 JSON：{{\"original\":\"<圖片或文字中的完整原文，保留原語言>\",\"translated\":\"<翻譯成{}的結果>\"}}。禁止 thinking、禁止解釋、禁止 markdown。",
        scenario.prompt,
        target_lang.display_name()
    )
}

fn build_chat_request(
    system_prompt: String,
    user_content: String,
    images: Option<Vec<String>>,
    stream: bool,
) -> ChatRequest {
    let mut user_parts = vec![ContentPart::Text { text: user_content }];
    if let Some(images) = images {
        for image_b64 in images {
            user_parts.push(ContentPart::ImageUrl {
                image_url: ImageUrl {
                    url: format!("data:image/png;base64,{image_b64}"),
                },
            });
        }
    }

    ChatRequest {
        model: CHAT_MODEL_NAME.to_string(),
        stream,
        messages: vec![
            ChatMessage::new_text("system", system_prompt),
            ChatMessage {
                role: "user".to_string(),
                content: user_parts,
            },
        ],
    }
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
        VlmError::Internal(format!("llama-server request failed: {err}"))
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
struct ChatRequest {
    model: String,
    stream: bool,
    messages: Vec<ChatMessage>,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: Vec<ContentPart>,
}

impl ChatMessage {
    fn new_text(role: &str, text: String) -> Self {
        Self {
            role: role.to_string(),
            content: vec![ContentPart::Text { text }],
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentPart {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
}

#[derive(Debug, Serialize)]
struct ImageUrl {
    url: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: Option<AssistantMessage>,
}

#[derive(Debug, Deserialize)]
struct AssistantMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatStreamChunk {
    choices: Vec<ChatStreamChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatStreamChoice {
    delta: ChatStreamDelta,
}

#[derive(Debug, Deserialize)]
struct ChatStreamDelta {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ModelOutput {
    original: String,
    translated: String,
}
