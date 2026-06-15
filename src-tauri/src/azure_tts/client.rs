use std::fmt;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::StatusCode;
use serde::de::Deserializer;
use serde::Deserialize;

use super::{TtsProvider, Voice, VoiceLevel};
use crate::tts::TtsError;
use crate::vlm::is_retryable_send_error;

pub struct AzureProvider {
    region: String,
    key: String,
    http: reqwest::Client,
    base_url: String,
}

impl AzureProvider {
    pub fn new(region: String, key: String) -> Self {
        let base_url = format!(
            "https://{}.tts.speech.microsoft.com",
            region.trim().to_ascii_lowercase()
        );
        Self::with_base_url(region, key, base_url)
    }

    fn with_base_url(region: String, key: String, base_url: String) -> Self {
        let http = reqwest::Client::builder()
            .pool_max_idle_per_host(0)
            .build()
            .unwrap_or_else(|err| {
                eprintln!("[azure-tts] failed to build client: {err}");
                reqwest::Client::new()
            });
        Self {
            region: region.trim().to_ascii_lowercase(),
            key,
            http,
            base_url,
        }
    }

    fn voices_url(&self) -> String {
        format!(
            "{}/cognitiveservices/voices/list",
            self.base_url.trim_end_matches('/')
        )
    }

    fn synthesize_url(&self) -> String {
        format!(
            "{}/cognitiveservices/v1",
            self.base_url.trim_end_matches('/')
        )
    }
}

impl fmt::Debug for AzureProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AzureProvider")
            .field("region", &self.region)
            .field("key", &"***")
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl TtsProvider for AzureProvider {
    async fn list_voices(&self, lang: &str) -> Result<Vec<Voice>, TtsError> {
        let response = self
            .http
            .get(self.voices_url())
            .timeout(Duration::from_secs(30))
            .header("Ocp-Apim-Subscription-Key", &self.key)
            .send()
            .await
            .map_err(|err| TtsError::Network(err.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let message = response.text().await.unwrap_or_else(|err| err.to_string());
            return Err(map_status(status, &self.region, message));
        }

        let body = response
            .text()
            .await
            .map_err(|err| TtsError::Network(err.to_string()))?;
        let raw = parse_azure_voices(&body)?;
        Ok(filter_azure_voices(raw, lang))
    }

    async fn test_connection(&self) -> Result<(), TtsError> {
        let voices = self.list_voices("en-US").await?;
        if voices.is_empty() {
            return Err(TtsError::VoiceNotFound("en-US".to_string()));
        }
        Ok(())
    }

    async fn synthesize(
        &self,
        text: &str,
        voice_id: &str,
        rate: f32,
        volume: f32,
    ) -> Result<Vec<u8>, TtsError> {
        let ssml = build_ssml(text, voice_id, rate, volume);
        let mut send_retried = false;
        let response = loop {
            match self
                .http
                .post(self.synthesize_url())
                .timeout(Duration::from_secs(10))
                .header("Ocp-Apim-Subscription-Key", &self.key)
                .header("Content-Type", "application/ssml+xml")
                .header(
                    "X-Microsoft-OutputFormat",
                    "audio-24khz-48kbitrate-mono-mp3",
                )
                .header("User-Agent", "capture2text-pro")
                .body(ssml.body.clone())
                .send()
                .await
            {
                Ok(response) => break response,
                Err(err) => {
                    let retryable = is_retryable_send_error(
                        err.is_connect(),
                        err.is_request(),
                        err.is_timeout(),
                    );
                    if !send_retried && retryable {
                        send_retried = true;
                        continue;
                    }
                    return Err(TtsError::Network(err.to_string()));
                }
            }
        };

        let status = response.status();
        if !status.is_success() {
            let message = response.text().await.unwrap_or_else(|err| err.to_string());
            return Err(match status.as_u16() {
                400 => TtsError::VoiceNotFound(voice_id.to_string()),
                _ => map_status(status, &self.region, message),
            });
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|err| TtsError::Network(err.to_string()))?;
        let billable = super::usage::count_billable_chars(
            &ssml.prosody_open,
            &ssml.escaped_text,
            ssml.prosody_close,
        );
        record_synthesis_usage(voice_id, billable);
        Ok(bytes.to_vec())
    }
}

fn map_status(status: StatusCode, region: &str, message: String) -> TtsError {
    match status.as_u16() {
        401 | 403 => TtsError::Auth,
        404 => TtsError::BadRegion(region.to_string()),
        429 => TtsError::Api {
            status: 429,
            message: "Rate limited".to_string(),
        },
        code => TtsError::Api {
            status: code,
            message,
        },
    }
}

#[cfg(not(test))]
fn record_synthesis_usage(voice_id: &str, chars: u64) {
    crate::window_state::record_usage(voice_id, chars);
}

#[cfg(test)]
fn record_synthesis_usage(_voice_id: &str, _chars: u64) {}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AzureVoice {
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    short_name: Option<String>,
    #[serde(default)]
    gender: Option<String>,
    #[serde(default)]
    locale: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_sample_rate_hertz")]
    sample_rate_hertz: Option<u32>,
}

impl AzureVoice {
    fn try_into_voice(self) -> Option<Voice> {
        let short_name = self.short_name?;
        let locale = self.locale?;
        let name = match self.display_name {
            Some(display_name) if !display_name.trim().is_empty() => display_name,
            _ => short_name.clone(),
        };
        let level = voice_level(&short_name);
        Some(Voice {
            id: short_name,
            name,
            locale,
            gender: self.gender.unwrap_or_default(),
            level,
            sample_rate: self.sample_rate_hertz.unwrap_or_default(),
        })
    }
}

fn parse_azure_voices(body: &str) -> Result<Vec<AzureVoice>, TtsError> {
    serde_json::from_str::<Vec<AzureVoice>>(body).map_err(|err| {
        let preview: String = body.chars().take(2000).collect();
        eprintln!("[azure-tts] invalid voices response body (first 2000 chars):\n{preview}");
        eprintln!(
            "[azure-tts] serde_json error: {err} (line {}, column {})",
            err.line(),
            err.column()
        );
        TtsError::Api {
            status: 200,
            message: format!("Invalid voices response: {err}"),
        }
    })
}

fn filter_azure_voices(raw: Vec<AzureVoice>, lang: &str) -> Vec<Voice> {
    let wanted_lang = lang.trim();
    raw.into_iter()
        .filter_map(AzureVoice::try_into_voice)
        .filter(|voice| wanted_lang.is_empty() || voice.locale.eq_ignore_ascii_case(wanted_lang))
        .collect()
}

fn deserialize_optional_sample_rate_hertz<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(match value {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::Number(number)) => {
            number.as_u64().and_then(|value| u32::try_from(value).ok())
        }
        Some(serde_json::Value::String(text)) => text.trim().parse::<u32>().ok(),
        _ => None,
    })
}

fn voice_level(short_name: &str) -> VoiceLevel {
    let lowered = short_name.to_ascii_lowercase();
    if lowered.contains("dragonhd") || lowered.contains("hd") {
        VoiceLevel::HighDefinition
    } else {
        VoiceLevel::Standard
    }
}

struct SsmlBody {
    body: String,
    prosody_open: String,
    escaped_text: String,
    prosody_close: &'static str,
}

fn build_ssml(text: &str, voice_id: &str, rate: f32, volume: f32) -> SsmlBody {
    let lang = lang_from_voice_id(voice_id);
    let escaped_text = escape_xml(text);
    let rate_pct = rate_percent(rate);
    let volume_pct = volume_percent(volume);
    let prosody_open = format!(r#"<prosody rate="{rate_pct}" volume="{volume_pct}">"#);
    let prosody_close = "</prosody>";
    let body = format!(
        r#"<speak version="1.0" xml:lang="{lang}"><voice name="{voice_id}">{prosody_open}{escaped_text}{prosody_close}</voice></speak>"#
    );
    SsmlBody {
        body,
        prosody_open,
        escaped_text,
        prosody_close,
    }
}

fn lang_from_voice_id(voice_id: &str) -> String {
    let mut parts = voice_id.split('-');
    match (parts.next(), parts.next()) {
        (Some(lang), Some(region)) if !lang.is_empty() && !region.is_empty() => {
            format!("{lang}-{region}")
        }
        _ => "en-US".to_string(),
    }
}

fn rate_percent(rate: f32) -> String {
    let pct = ((rate.clamp(0.5, 2.0) - 1.0) * 100.0).round() as i32;
    if pct > 0 {
        format!("+{pct}%")
    } else {
        format!("{pct}%")
    }
}

fn volume_percent(volume: f32) -> String {
    let pct = ((volume.clamp(0.5, 2.0) - 1.0) * 100.0).round() as i32;
    if pct > 0 {
        format!("+{pct}%")
    } else {
        format!("{pct}%")
    }
}

fn escape_xml(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod azure_voices {
    use wiremock::matchers::{body_string_contains, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    #[tokio::test]
    async fn azure_voices_list_filters_locale_and_maps_fields() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cognitiveservices/voices/list"))
            .and(header("Ocp-Apim-Subscription-Key", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"[
                    {
                        "Name": "Microsoft Server Speech Text to Speech Voice (zh-TW, HsiaoChenNeural)",
                        "DisplayName": "HsiaoChen",
                        "LocalName": "HsiaoChen",
                        "ShortName": "zh-TW-HsiaoChenNeural",
                        "Gender": "Female",
                        "Locale": "zh-TW",
                        "SampleRateHertz": "24000",
                        "VoiceType": "Neural",
                        "Status": "GA"
                    },
                    {
                        "Name": "Microsoft Server Speech Text to Speech Voice (en-US, Ava:DragonHDLatestNeural)",
                        "DisplayName": "Ava",
                        "LocalName": "Ava",
                        "ShortName": "en-US-Ava:DragonHDLatestNeural",
                        "Gender": "Female",
                        "Locale": "en-US",
                        "SampleRateHertz": "48000",
                        "VoiceType": "Neural",
                        "Status": "GA"
                    }
                ]"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let provider = AzureProvider::with_base_url(
            "eastasia".to_string(),
            "test-key".to_string(),
            server.uri(),
        );
        let voices = provider.list_voices("zh-TW").await.unwrap();

        assert_eq!(voices.len(), 1);
        assert_eq!(voices[0].id, "zh-TW-HsiaoChenNeural");
        assert_eq!(voices[0].name, "HsiaoChen");
        assert_eq!(voices[0].locale, "zh-TW");
        assert_eq!(voices[0].gender, "Female");
        assert_eq!(voices[0].level, VoiceLevel::Standard);
        assert_eq!(voices[0].sample_rate, 24000);
    }

    #[test]
    fn azure_voices_missing_optional_fields_do_not_break_parse() {
        let body = r#"
            [
                {
                    "DisplayName": "Preview Voice",
                    "ShortName": "zh-TW-PreviewNeural",
                    "Locale": "zh-TW"
                },
                {
                    "DisplayName": "HsiaoChen",
                    "ShortName": "zh-TW-HsiaoChenNeural",
                    "Gender": "Female",
                    "Locale": "zh-TW",
                    "SampleRateHertz": "24000"
                }
            ]
        "#;

        let voices = filter_azure_voices(parse_azure_voices(body).unwrap(), "zh-TW");

        assert_eq!(voices.len(), 2);
        assert_eq!(voices[0].id, "zh-TW-PreviewNeural");
        assert_eq!(voices[0].name, "Preview Voice");
        assert_eq!(voices[0].locale, "zh-TW");
        assert_eq!(voices[0].gender, "");
        assert_eq!(voices[0].level, VoiceLevel::Standard);
        assert_eq!(voices[0].sample_rate, 0);
        assert_eq!(voices[1].id, "zh-TW-HsiaoChenNeural");
        assert_eq!(voices[1].name, "HsiaoChen");
        assert_eq!(voices[1].locale, "zh-TW");
        assert_eq!(voices[1].gender, "Female");
        assert_eq!(voices[1].level, VoiceLevel::Standard);
        assert_eq!(voices[1].sample_rate, 24000);
    }

    #[tokio::test]
    async fn azure_voices_list_maps_401_to_auth() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cognitiveservices/voices/list"))
            .respond_with(ResponseTemplate::new(401).set_body_string("bad key"))
            .mount(&server)
            .await;

        let provider = AzureProvider::with_base_url(
            "eastasia".to_string(),
            "bad-key".to_string(),
            server.uri(),
        );
        let err = provider.list_voices("zh-TW").await.unwrap_err();

        assert!(matches!(err, TtsError::Auth));
    }

    #[tokio::test]
    async fn azure_synthesize_posts_ssml_and_returns_audio_bytes() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/cognitiveservices/v1"))
            .and(header("Ocp-Apim-Subscription-Key", "test-key"))
            .and(header(
                "X-Microsoft-OutputFormat",
                "audio-24khz-48kbitrate-mono-mp3",
            ))
            .and(body_string_contains(
                r#"<voice name="zh-TW-HsiaoChenNeural">"#,
            ))
            .and(body_string_contains(
                "hello &amp; &lt;world&gt; &quot;quoted&quot; &apos;ok&apos;",
            ))
            .and(body_string_contains(r#"<prosody rate="+20%" volume="0%">"#))
            .and(body_string_contains(r#"volume="0%""#))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![1, 2, 3, 4]))
            .mount(&server)
            .await;

        let provider = AzureProvider::with_base_url(
            "eastasia".to_string(),
            "test-key".to_string(),
            server.uri(),
        );
        let bytes = provider
            .synthesize(
                "hello & <world> \"quoted\" 'ok'",
                "zh-TW-HsiaoChenNeural",
                1.2,
                1.0,
            )
            .await
            .unwrap();

        assert_eq!(bytes, vec![1, 2, 3, 4]);
    }

    #[tokio::test]
    async fn azure_synthesize_maps_401_to_auth() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/cognitiveservices/v1"))
            .respond_with(ResponseTemplate::new(401).set_body_string("bad key"))
            .mount(&server)
            .await;

        let provider = AzureProvider::with_base_url(
            "eastasia".to_string(),
            "bad-key".to_string(),
            server.uri(),
        );
        let err = provider
            .synthesize("hello", "zh-TW-HsiaoChenNeural", 1.0, 1.0)
            .await
            .unwrap_err();

        assert!(matches!(err, TtsError::Auth));
    }
}
