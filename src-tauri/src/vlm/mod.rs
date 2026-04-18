use std::io;
use std::time::Instant;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};

const OLLAMA_CHAT_URL: &str = "http://localhost:11434/api/chat";
const OLLAMA_MODEL: &str = "qwen3-vl:8b-instruct";

#[derive(Debug, Clone, Copy)]
pub enum TargetLang {
    Chinese,
    English,
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

pub fn ocr_and_translate(png_bytes: &[u8], target_lang: TargetLang) -> io::Result<VlmOutput> {
    let started_at = Instant::now();
    let image_b64 = STANDARD.encode(png_bytes);

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
