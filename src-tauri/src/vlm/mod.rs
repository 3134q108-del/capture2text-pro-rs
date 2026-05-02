use std::io::{self, BufRead, BufReader};
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::atomic::{AtomicU64, Ordering};
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
    VlmRuntimeDown,
    #[error("llama-server returned HTTP {status}: {body}")]
    VlmRuntimeHttpError { status: u16, body: String },
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
    #[error("cancelled by newer request")]
    Cancelled,
}

impl From<io::Error> for VlmError {
    fn from(err: io::Error) -> Self {
        VlmError::Internal(err.to_string())
    }
}

#[derive(Debug)]
pub enum HealthStatus {
    Healthy,
    VlmRuntimeDown,
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
            Self::VlmRuntimeDown => "vlm-runtime-down",
            Self::ModelMissing { .. } => "model-missing",
            Self::Unknown(_) => "unknown",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::Healthy => "OK".to_string(),
            Self::VlmRuntimeDown => "llama.cpp runtime is not ready".to_string(),
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

pub enum VlmJob {
    OcrAndTranslate {
        png_bytes: Vec<u8>,
        native_lang: String,
        target_lang: String,
        source: &'static str,
        seq: u64,
    },
    TranslateText {
        text: String,
        native_lang: String,
        target_lang: String,
        source: &'static str,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct VlmEventPayload {
    pub source: String,
    pub status: String,
    pub original: String,
    pub translated: String,
    pub src_lang: Option<String>,
    pub duration_ms: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VlmPartialEventPayload {
    pub source: String,
    pub original: String,
    pub translated: String,
    pub src_lang: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PartialOutput {
    pub raw_accumulated: String,
    pub original: Option<String>,
    pub translated: Option<String>,
    pub src_lang: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct VlmOutput {
    pub original: String,
    pub translated: String,
    pub src_lang: Option<String>,
    pub duration_ms: u64,
    pub seq: Option<u64>,
}

static VLM_RUNTIME: OnceLock<VlmRuntime> = OnceLock::new();
static ACTIVE_VLM_SRC_LANG: OnceLock<Mutex<Option<String>>> = OnceLock::new();
static LATEST_OCR_SEQ: AtomicU64 = AtomicU64::new(0);

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
                let lang_code = job_model_lang(&job);
                if let Err(err) = llama_runtime::ensure_model_for_lang(lang_code) {
                    let message = format!(
                        "switch model for lang {lang_code} failed: {err}"
                    );
                    eprintln!("[vlm] source={} failed: {}", source_label, message);
                    set_active_src_lang(None);
                    emit_vlm_event(
                        &app_handle,
                        VlmEventPayload {
                            source: source_label,
                            status: "error".to_string(),
                            original: String::new(),
                            translated: String::new(),
                            src_lang: None,
                            duration_ms: 0,
                            error: Some(message),
                        },
                    );
                    continue;
                }

                let (source, result) = match job {
                    VlmJob::OcrAndTranslate {
                        png_bytes,
                        native_lang,
                        target_lang,
                        source,
                        seq,
                    } => {
                        if is_seq_cancelled(Some(seq)) {
                            continue;
                        }
                        let source_label = source.to_string();
                        let source_for_partial = source_label.clone();
                        let result = ocr_and_translate_streaming(
                            &png_bytes,
                            &native_lang,
                            target_lang,
                            Some(seq),
                            |partial| {
                                emit_vlm_partial_event(
                                    &app_handle,
                                    VlmPartialEventPayload {
                                        source: source_for_partial.clone(),
                                        original: partial.original.clone().unwrap_or_default(),
                                        translated: partial.translated.clone().unwrap_or_default(),
                                        src_lang: partial.src_lang.clone(),
                                    },
                                );
                            },
                        );
                        (source_label, result)
                    }
                    VlmJob::TranslateText {
                        text,
                        native_lang,
                        target_lang,
                        source,
                    } => {
                        let source_label = source.to_string();
                        let source_for_partial = source_label.clone();
                        let result = translate_text_streaming(&text, &native_lang, &target_lang, |partial| {
                            emit_vlm_partial_event(
                                &app_handle,
                                VlmPartialEventPayload {
                                    source: source_for_partial.clone(),
                                    original: partial.original.clone().unwrap_or_default(),
                                    translated: partial.translated.clone().unwrap_or_default(),
                                    src_lang: partial.src_lang.clone(),
                                },
                            );
                        });
                        (source_label, result)
                    }
                };

                match result {
                    Ok(out) => {
                        if is_seq_cancelled(out.seq) {
                            continue;
                        }
                        println!("[vlm] source={} original: {}", source, out.original);
                        println!("[vlm] source={} translated: {}", source, out.translated);
                        println!("[vlm] source={} duration_ms: {}", source, out.duration_ms);
                        eprintln!("[vlm] source={} src_lang: {:?}", source, &out.src_lang);
                        set_active_src_lang(out.src_lang.clone());
                        emit_vlm_event(
                            &app_handle,
                            VlmEventPayload {
                                source,
                                status: "success".to_string(),
                                original: out.original,
                                translated: out.translated,
                                src_lang: out.src_lang,
                                duration_ms: out.duration_ms,
                                error: None,
                            },
                        );
                    }
                    Err(err) => {
                        if matches!(err, VlmError::Cancelled) {
                            continue;
                        }
                        eprintln!("[vlm] source={} failed: {err}", source);
                        set_active_src_lang(None);
                        emit_vlm_event(
                            &app_handle,
                            VlmEventPayload {
                                source,
                                status: "error".to_string(),
                                original: String::new(),
                                translated: String::new(),
                                src_lang: None,
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

pub fn active_src_lang() -> Option<String> {
    let slot = ACTIVE_VLM_SRC_LANG.get_or_init(|| Mutex::new(None));
    slot.lock().ok().and_then(|guard| guard.clone())
}

fn set_active_src_lang(lang: Option<String>) {
    let slot = ACTIVE_VLM_SRC_LANG.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = slot.lock() {
        *guard = lang;
    }
}

pub fn try_submit_ocr(
    png_bytes: Vec<u8>,
    native_lang: String,
    target_lang: String,
    source: &'static str,
    seq: u64,
) {
    LATEST_OCR_SEQ.store(seq, Ordering::SeqCst);
    try_submit(VlmJob::OcrAndTranslate {
        png_bytes,
        native_lang,
        target_lang,
        source,
        seq,
    });
}

pub fn try_submit_text(
    text: String,
    native_lang: String,
    target_lang: String,
    source: &'static str,
) {
    try_submit(VlmJob::TranslateText {
        text,
        native_lang,
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

fn job_model_lang(job: &VlmJob) -> &str {
    match job {
        VlmJob::OcrAndTranslate { native_lang, .. } => native_lang.as_str(),
        VlmJob::TranslateText { target_lang, .. } => target_lang.as_str(),
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
            eprintln!("[emit] ensure_result_window_visible: window creation failed: {err}");
            return;
        }
    };

    if let Err(err) = window.unminimize() {
        eprintln!("[emit] unminimize failed: {err}");
    }

    if let Err(err) = window.show() {
        eprintln!("[emit] show failed: {err}");
        return;
    }

    if let Err(err) = window.set_focus() {
        eprintln!("[emit] set_focus failed: {err}");
    }

    thread::sleep(Duration::from_millis(50));
}

pub fn check_health() -> HealthStatus {
    if llama_runtime::supervisor::is_healthy() {
        HealthStatus::Healthy
    } else {
        HealthStatus::VlmRuntimeDown
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
            response_format: None,
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
    native_lang: &str,
    target_lang: String,
    seq: Option<u64>,
    mut on_partial: F,
) -> VlmResult<VlmOutput> {
    let png_bytes = ensure_min_dimension(png_bytes)?;
    let started_at = Instant::now();
    let image_b64 = STANDARD.encode(&png_bytes);
    let mode = crate::window_state::translation_mode();
    let system_prompt = match mode {
        crate::window_state::TranslationMode::Smart => {
            build_smart_system_prompt(native_lang, &target_lang)
        }
        crate::window_state::TranslationMode::Direct => build_direct_system_prompt(&target_lang),
    };
    let request = build_chat_request(
        system_prompt,
        "Extract text from the image and translate per the rules.".to_string(),
        Some(vec![image_b64]),
        true,
    );

    let (raw_accumulated, duration_ns) = run_streaming_request(request, seq, |raw| {
        if is_seq_cancelled(seq) {
            return;
        }
        on_partial(&PartialOutput {
            raw_accumulated: raw.to_string(),
            original: extract_partial_json_string(raw, "original"),
            translated: extract_partial_json_string(raw, "translated"),
            src_lang: extract_partial_json_string(raw, "src_lang"),
        });
    })?;

    let parsed = parse_model_output(&raw_accumulated)?;
    let duration_ms = duration_ns
        .map(|ns| ns / 1_000_000)
        .unwrap_or_else(|| started_at.elapsed().as_millis() as u64);
    Ok(VlmOutput {
        original: parsed.original,
        translated: parsed.translated,
        src_lang: parsed.src_lang,
        duration_ms,
        seq,
    })
}

pub fn translate_text_streaming<F: FnMut(&PartialOutput)>(
    text: &str,
    native_lang: &str,
    target_lang: &str,
    on_partial: F,
) -> VlmResult<VlmOutput> {
    let effective_target = decide_effective_target(active_src_lang().as_deref(), native_lang, target_lang);
    translate_text_to_lang_streaming(text, &effective_target, None, on_partial)
}

pub fn ocr_and_translate(
    png_bytes: &[u8],
    native_lang: &str,
    target_lang: &str,
) -> VlmResult<VlmOutput> {
    ocr_and_translate_streaming(png_bytes, native_lang, target_lang.to_string(), None, |_| {})
}

fn translate_text_to_lang_streaming<F: FnMut(&PartialOutput)>(
    text: &str,
    target_lang: &str,
    seq: Option<u64>,
    mut on_partial: F,
) -> VlmResult<VlmOutput> {
    let started_at = Instant::now();
    let request = build_chat_request(
        build_system_prompt(target_lang),
        text.to_string(),
        None,
        true,
    );

    let (raw_accumulated, duration_ns) = run_streaming_request(request, seq, |raw| {
        on_partial(&PartialOutput {
            raw_accumulated: raw.to_string(),
            original: Some(text.to_string()),
            translated: extract_partial_json_string(raw, "translated"),
            src_lang: extract_partial_json_string(raw, "src_lang"),
        });
    })?;

    let parsed = parse_model_output(&raw_accumulated)?;
    let duration_ms = duration_ns
        .map(|ns| ns / 1_000_000)
        .unwrap_or_else(|| started_at.elapsed().as_millis() as u64);
    Ok(VlmOutput {
        original: text.to_string(),
        translated: parsed.translated,
        src_lang: parsed.src_lang,
        duration_ms,
        seq,
    })
}

fn run_streaming_request<F: FnMut(&str)>(
    request: ChatRequest,
    seq: Option<u64>,
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
        return Err(VlmError::VlmRuntimeHttpError {
            status: status.as_u16(),
            body: raw,
        });
    }

    let mut raw_accumulated = String::new();
    let final_duration_ns: Option<u64> = None;
    let reader = BufReader::new(response);
    for line in reader.lines() {
        if is_seq_cancelled(seq) {
            return Err(VlmError::Cancelled);
        }
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

fn build_system_prompt(target_lang: &str) -> String {
    let scenario = scenarios::current_scenario();
    let language_name = crate::languages::by_code(target_lang)
        .map(|lang| lang.english_name)
        .unwrap_or("English");
    let language_codes = crate::languages::all()
        .iter()
        .map(|lang| lang.code.as_str())
        .collect::<Vec<_>>()
        .join(" | ");
    format!(
        "{}\n\nReturn strict JSON only: {{\"original\":\"<full source text>\",\"translated\":\"<translation in {}>\",\"src_lang\":\"<BCP-47 from: {} | other>\"}}. No thinking. No explanation. No markdown.",
        scenario.prompt, language_name, language_codes
    )
}

fn build_smart_system_prompt(native_lang: &str, target_lang: &str) -> String {
    let scenario = scenarios::current_scenario();
    let native = crate::languages::by_code(native_lang)
        .or_else(|| crate::languages::by_code("zh-TW"))
        .expect("native language fallback");
    let target = crate::languages::by_code(target_lang)
        .or_else(|| crate::languages::by_code("en-US"))
        .expect("target language fallback");
    let language_codes = crate::languages::all()
        .iter()
        .map(|lang| lang.code.as_str())
        .collect::<Vec<_>>()
        .join(" | ");

    format!(
        "{scenario_prompt}\n\n\
         You are an OCR + smart translator. The user's native language is {native_name} ({native_code}). Their target study language is {target_name} ({target_code}).\n\n\
         Step 1: OCR the image text exactly.\n\
         Step 2: Identify the source language.\n\
         Step 3: Apply translation direction:\n\
         - If source is {native_name} ({native_code}), translate to {target_name} ({target_code}).\n\
         - For any other source language (including {target_name}), translate to {native_name} ({native_code}).\n\n\
         Return strict JSON only: {{\"original\":\"<exact OCR text>\",\"translated\":\"<translation per Step 3>\",\"src_lang\":\"<BCP-47 from: {codes} | other>\"}}. No thinking. No explanation. No markdown.",
        scenario_prompt = scenario.prompt,
        native_name = native.english_name,
        native_code = native.code.as_str(),
        target_name = target.english_name,
        target_code = target.code.as_str(),
        codes = language_codes,
    )
}

fn build_direct_system_prompt(target_lang: &str) -> String {
    let scenario = scenarios::current_scenario();
    let target = crate::languages::by_code(target_lang)
        .or_else(|| crate::languages::by_code("en-US"))
        .expect("target language fallback");
    let language_codes = crate::languages::all()
        .iter()
        .map(|lang| lang.code.as_str())
        .collect::<Vec<_>>()
        .join(" | ");

    format!(
        "{scenario_prompt}\n\n\
         You are an OCR translator. Translate the image text to {target_name} ({target_code}), regardless of source language.\n\n\
         Step 1: OCR the image text exactly.\n\
         Step 2: Translate the text to {target_name} ({target_code}). If the source is already in {target_name}, return it unchanged.\n\
         Step 3: Identify the source language for src_lang field.\n\n\
         Return strict JSON only: {{\"original\":\"<exact OCR text>\",\"translated\":\"<translation in {target_name}>\",\"src_lang\":\"<BCP-47 from: {codes} | other>\"}}. No thinking. No explanation. No markdown.",
        scenario_prompt = scenario.prompt,
        target_name = target.english_name,
        target_code = target.code.as_str(),
        codes = language_codes,
    )
}

fn normalize_lang_code(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let canonical_guess = match trimmed.to_ascii_lowercase().as_str() {
        "zh" | "zh-tw" => "zh-TW",
        "zh-cn" => "zh-CN",
        "en" | "en-us" => "en-US",
        "ja" | "ja-jp" => "ja-JP",
        "ko" | "ko-kr" => "ko-KR",
        "fr" | "fr-fr" => "fr-FR",
        "de" | "de-de" => "de-DE",
        _ => trimmed,
    };

    crate::languages::all()
        .iter()
        .find(|lang| lang.code.as_str().eq_ignore_ascii_case(canonical_guess))
        .map(|lang| lang.code.as_str().to_string())
}

fn decide_effective_target(src_lang: Option<&str>, native_lang: &str, target_lang: &str) -> String {
    let native = normalize_lang_code(native_lang).unwrap_or_else(|| "zh-TW".to_string());
    let target = normalize_lang_code(target_lang).unwrap_or_else(|| "en-US".to_string());
    match src_lang.and_then(normalize_lang_code) {
        Some(src) if src == native => target,
        Some(_) => native,
        None => native,
    }
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
        response_format: Some(ResponseFormat {
            format_type: ResponseFormatType::JsonObject,
        }),
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

    let Some(json_body) = extract_first_json_object(content) else {
        let translated = content.trim().to_string();
        eprintln!(
            "[vlm] lenient parse used: model returned non-JSON, raw len={}",
            content.len()
        );
        return Ok(ModelOutput {
            original: String::new(),
            translated,
            src_lang: None,
        });
    };

    serde_json::from_str::<ModelOutput>(json_body).map_err(|err| VlmError::ResponseDecode {
        raw: content.to_string(),
        source_error: format!("model JSON parse failed: {err}"),
    })
}

fn map_reqwest_send_error(err: reqwest::Error) -> VlmError {
    if err.is_timeout() {
        VlmError::Timeout(REQUEST_TIMEOUT_MS)
    } else if err.is_connect() {
        VlmError::VlmRuntimeDown
    } else {
        VlmError::Internal(format!("llama-server request failed: {err}"))
    }
}

fn is_seq_cancelled(seq: Option<u64>) -> bool {
    match seq {
        Some(current) => current < LATEST_OCR_SEQ.load(Ordering::SeqCst),
        None => false,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,
    messages: Vec<ChatMessage>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum ResponseFormatType {
    JsonObject,
}

#[derive(Debug, Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    format_type: ResponseFormatType,
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
    #[serde(default)]
    src_lang: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_strict_json_passes() {
        let parsed = parse_model_output(r#"{"original":"a","translated":"b","src_lang":"en-US"}"#)
            .expect("strict JSON should parse");
        assert_eq!(parsed.original, "a");
        assert_eq!(parsed.translated, "b");
        assert_eq!(parsed.src_lang.as_deref(), Some("en-US"));
    }

    #[test]
    fn parse_extracts_first_json_object() {
        let parsed = parse_model_output(r#"blah {"original":"a","translated":"b"} blah"#)
            .expect("should extract embedded JSON");
        assert_eq!(parsed.original, "a");
        assert_eq!(parsed.translated, "b");
        assert_eq!(parsed.src_lang, None);
    }

    #[test]
    fn parse_lenient_fallback_when_no_braces() {
        let parsed = parse_model_output("This fix is permanent.")
            .expect("non-JSON text should use lenient fallback");
        assert_eq!(parsed.original, "");
        assert_eq!(parsed.translated, "This fix is permanent.");
        assert_eq!(parsed.src_lang, None);
    }

    #[test]
    fn parse_lenient_strips_whitespace() {
        let parsed = parse_model_output("  hello \n")
            .expect("lenient fallback should trim whitespace");
        assert_eq!(parsed.original, "");
        assert_eq!(parsed.translated, "hello");
        assert_eq!(parsed.src_lang, None);
    }

    #[test]
    fn parse_returns_error_for_malformed_json_with_braces() {
        let err = parse_model_output("{not valid}").expect_err("malformed JSON should remain error");
        match err {
            VlmError::ResponseDecode { .. } => {}
            other => panic!("expected ResponseDecode, got {other:?}"),
        }
    }

}


