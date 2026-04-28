use chrono::{Datelike, Utc};

pub fn count_billable_chars(prosody_open: &str, escaped_text: &str, prosody_close: &str) -> u64 {
    let markup =
        prosody_open.chars().count() as u64 + prosody_close.chars().count() as u64;
    let text = escaped_text
        .chars()
        .map(|ch| if is_cjk_ideograph(ch) { 2 } else { 1 })
        .sum::<u64>();
    markup + text
}

pub fn is_cjk_ideograph(ch: char) -> bool {
    let code = ch as u32;
    matches!(
        code,
        0x4E00..=0x9FFF
            | 0x3400..=0x4DBF
            | 0x20000..=0x2A6DF
            | 0x2A700..=0x2B73F
            | 0x2B740..=0x2B81F
            | 0x2B820..=0x2CEAF
            | 0x2CEB0..=0x2EBEF
            | 0xF900..=0xFAFF
    )
}

pub fn is_hd_voice(voice_id: &str) -> bool {
    let voice = voice_id.to_ascii_lowercase();
    voice.contains("dragonhd") || voice.contains("hdlatest") || voice.contains(":hd")
}

pub fn current_month() -> String {
    let now = Utc::now();
    format!("{:04}-{:02}", now.year(), now.month())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_billable_chars_counts_markup_escape_and_cjk() {
        let prosody_open = r#"<prosody rate="+20%">"#;
        let escaped_text = "春&amp;A";
        let prosody_close = "</prosody>";

        let expected = prosody_open.chars().count() as u64
            + prosody_close.chars().count() as u64
            + 2
            + 5
            + 1;
        assert_eq!(
            count_billable_chars(prosody_open, escaped_text, prosody_close),
            expected
        );
    }

    #[test]
    fn is_hd_voice_detects_azure_hd_patterns() {
        assert!(is_hd_voice("en-US-Ava:DragonHDLatestNeural"));
        assert!(is_hd_voice("en-US-AndrewHDLatestNeural"));
        assert!(is_hd_voice("en-US-Ava:HDLatestNeural"));
        assert!(!is_hd_voice("en-US-AvaMultilingualNeural"));
    }
}
