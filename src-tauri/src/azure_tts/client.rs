use std::fmt;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::StatusCode;
use serde::Deserialize;

use super::{TtsProvider, Voice, VoiceLevel};
use crate::tts::TtsError;

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
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|err| {
                eprintln!("[azure-tts] failed to build timeout client: {err}");
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
            .header("Ocp-Apim-Subscription-Key", &self.key)
            .send()
            .await
            .map_err(|err| TtsError::Network(err.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let message = response.text().await.unwrap_or_else(|err| err.to_string());
            return Err(map_status(status, &self.region, message));
        }

        let raw = response
            .json::<Vec<AzureVoice>>()
            .await
            .map_err(|err| TtsError::Api {
                status: 200,
                message: format!("Invalid voices response: {err}"),
            })?;
        let mut voices = Vec::new();
        for voice in raw {
            if lang.trim().is_empty() || voice.locale.eq_ignore_ascii_case(lang.trim()) {
                voices.push(voice.try_into_voice()?);
            }
        }
        Ok(voices)
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
    ) -> Result<Vec<u8>, TtsError> {
        let ssml = build_ssml(text, voice_id, rate);
        let response = self
            .http
            .post(self.synthesize_url())
            .header("Ocp-Apim-Subscription-Key", &self.key)
            .header("Content-Type", "application/ssml+xml")
            .header(
                "X-Microsoft-OutputFormat",
                "audio-24khz-48kbitrate-mono-mp3",
            )
            .header("User-Agent", "capture2text-pro")
            .body(ssml)
            .send()
            .await
            .map_err(|err| TtsError::Network(err.to_string()))?;

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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AzureVoice {
    display_name: String,
    short_name: String,
    gender: String,
    locale: String,
    sample_rate_hertz: String,
}

impl AzureVoice {
    fn try_into_voice(self) -> Result<Voice, TtsError> {
        let sample_rate = self
            .sample_rate_hertz
            .parse::<u32>()
            .map_err(|err| TtsError::Api {
                status: 200,
                message: format!(
                    "Invalid SampleRateHertz for {}: {} ({err})",
                    self.short_name, self.sample_rate_hertz
                ),
            })?;
        let level = voice_level(&self.short_name);
        Ok(Voice {
            id: self.short_name,
            name: self.display_name,
            locale: self.locale,
            gender: self.gender,
            level,
            sample_rate,
        })
    }
}

fn voice_level(short_name: &str) -> VoiceLevel {
    let lowered = short_name.to_ascii_lowercase();
    if lowered.contains("dragonhd") || lowered.contains("hd") {
        VoiceLevel::HighDefinition
    } else {
        VoiceLevel::Standard
    }
}

fn build_ssml(text: &str, voice_id: &str, rate: f32) -> String {
    let lang = lang_from_voice_id(voice_id);
    let escaped_text = escape_xml(text);
    let rate_pct = rate_percent(rate);
    format!(
        r#"<speak version="1.0" xml:lang="{lang}"><voice name="{voice_id}"><prosody rate="{rate_pct}">{escaped_text}</prosody></voice></speak>"#
    )
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
            .and(body_string_contains(r#"<prosody rate="+20%">"#))
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
            .synthesize("hello", "zh-TW-HsiaoChenNeural", 1.0)
            .await
            .unwrap_err();

        assert!(matches!(err, TtsError::Auth));
    }
}
