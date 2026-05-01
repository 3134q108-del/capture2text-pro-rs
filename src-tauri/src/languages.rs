#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LangCode(&'static str);

impl LangCode {
    pub const fn new(code: &'static str) -> Self {
        Self(code)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    S,
    A,
    B,
    C,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptDirection {
    Ltr,
    Rtl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Language {
    pub code: LangCode,
    pub native_name: &'static str,
    pub english_name: &'static str,
    pub tier: Tier,
    pub default_voice_id: &'static str,
    pub script_direction: ScriptDirection,
}

const FALLBACK_VOICE: &str = "en-US-AvaNeural";

pub const LANGUAGES: &[Language] = &[
    Language { code: LangCode::new("zh-CN"), native_name: "简体中文", english_name: "Chinese (Simplified)", tier: Tier::S, default_voice_id: "zh-CN-XiaoxiaoNeural", script_direction: ScriptDirection::Ltr },
    Language { code: LangCode::new("zh-TW"), native_name: "繁體中文", english_name: "Chinese (Traditional)", tier: Tier::S, default_voice_id: "zh-TW-HsiaoChenNeural", script_direction: ScriptDirection::Ltr },
    Language { code: LangCode::new("en-US"), native_name: "English", english_name: "English", tier: Tier::S, default_voice_id: "en-US-AvaMultilingualNeural", script_direction: ScriptDirection::Ltr },
    Language { code: LangCode::new("ja-JP"), native_name: "日本語", english_name: "Japanese", tier: Tier::S, default_voice_id: "ja-JP-NanamiNeural", script_direction: ScriptDirection::Ltr },
    Language { code: LangCode::new("ko-KR"), native_name: "한국어", english_name: "Korean", tier: Tier::S, default_voice_id: "ko-KR-SunHiNeural", script_direction: ScriptDirection::Ltr },
    Language { code: LangCode::new("fr-FR"), native_name: "Francais", english_name: "French", tier: Tier::A, default_voice_id: "fr-FR-VivienneMultilingualNeural", script_direction: ScriptDirection::Ltr },
    Language { code: LangCode::new("de-DE"), native_name: "Deutsch", english_name: "German", tier: Tier::A, default_voice_id: "de-DE-SeraphinaMultilingualNeural", script_direction: ScriptDirection::Ltr },
    Language { code: LangCode::new("es-ES"), native_name: "Espanol", english_name: "Spanish", tier: Tier::A, default_voice_id: "es-ES-XimenaNeural", script_direction: ScriptDirection::Ltr },
    Language { code: LangCode::new("pt-PT"), native_name: "Portugues", english_name: "Portuguese", tier: Tier::A, default_voice_id: "pt-PT-RaquelNeural", script_direction: ScriptDirection::Ltr },
    Language { code: LangCode::new("it-IT"), native_name: "Italiano", english_name: "Italian", tier: Tier::A, default_voice_id: "it-IT-IsabellaNeural", script_direction: ScriptDirection::Ltr },
    Language { code: LangCode::new("ru-RU"), native_name: "Русский", english_name: "Russian", tier: Tier::A, default_voice_id: "ru-RU-SvetlanaNeural", script_direction: ScriptDirection::Ltr },
    Language { code: LangCode::new("vi-VN"), native_name: "Tieng Viet", english_name: "Vietnamese", tier: Tier::A, default_voice_id: "vi-VN-HoaiMyNeural", script_direction: ScriptDirection::Ltr },
    Language { code: LangCode::new("ar-SA"), native_name: "العربية", english_name: "Arabic", tier: Tier::B, default_voice_id: FALLBACK_VOICE, script_direction: ScriptDirection::Rtl },
    Language { code: LangCode::new("id-ID"), native_name: "Bahasa Indonesia", english_name: "Indonesian", tier: Tier::B, default_voice_id: FALLBACK_VOICE, script_direction: ScriptDirection::Ltr },
    Language { code: LangCode::new("th-TH"), native_name: "ไทย", english_name: "Thai", tier: Tier::B, default_voice_id: FALLBACK_VOICE, script_direction: ScriptDirection::Ltr },
    Language { code: LangCode::new("hi-IN"), native_name: "हिन्दी", english_name: "Hindi", tier: Tier::B, default_voice_id: FALLBACK_VOICE, script_direction: ScriptDirection::Ltr },
    Language { code: LangCode::new("el-GR"), native_name: "Ελληνικα", english_name: "Greek", tier: Tier::B, default_voice_id: FALLBACK_VOICE, script_direction: ScriptDirection::Ltr },
    Language { code: LangCode::new("he-IL"), native_name: "עברית", english_name: "Hebrew", tier: Tier::B, default_voice_id: FALLBACK_VOICE, script_direction: ScriptDirection::Rtl },
    Language { code: LangCode::new("tr-TR"), native_name: "Turkce", english_name: "Turkish", tier: Tier::B, default_voice_id: FALLBACK_VOICE, script_direction: ScriptDirection::Ltr },
    Language { code: LangCode::new("pl-PL"), native_name: "Polski", english_name: "Polish", tier: Tier::B, default_voice_id: FALLBACK_VOICE, script_direction: ScriptDirection::Ltr },
    Language { code: LangCode::new("nl-NL"), native_name: "Nederlands", english_name: "Dutch", tier: Tier::C, default_voice_id: FALLBACK_VOICE, script_direction: ScriptDirection::Ltr },
    Language { code: LangCode::new("uk-UA"), native_name: "Українська", english_name: "Ukrainian", tier: Tier::C, default_voice_id: FALLBACK_VOICE, script_direction: ScriptDirection::Ltr },
    Language { code: LangCode::new("cs-CZ"), native_name: "Cestina", english_name: "Czech", tier: Tier::C, default_voice_id: FALLBACK_VOICE, script_direction: ScriptDirection::Ltr },
    Language { code: LangCode::new("sv-SE"), native_name: "Svenska", english_name: "Swedish", tier: Tier::C, default_voice_id: FALLBACK_VOICE, script_direction: ScriptDirection::Ltr },
    Language { code: LangCode::new("da-DK"), native_name: "Dansk", english_name: "Danish", tier: Tier::C, default_voice_id: FALLBACK_VOICE, script_direction: ScriptDirection::Ltr },
    Language { code: LangCode::new("no-NO"), native_name: "Norsk", english_name: "Norwegian", tier: Tier::C, default_voice_id: FALLBACK_VOICE, script_direction: ScriptDirection::Ltr },
    Language { code: LangCode::new("fi-FI"), native_name: "Suomi", english_name: "Finnish", tier: Tier::C, default_voice_id: FALLBACK_VOICE, script_direction: ScriptDirection::Ltr },
    Language { code: LangCode::new("hu-HU"), native_name: "Magyar", english_name: "Hungarian", tier: Tier::C, default_voice_id: FALLBACK_VOICE, script_direction: ScriptDirection::Ltr },
    Language { code: LangCode::new("ro-RO"), native_name: "Romana", english_name: "Romanian", tier: Tier::C, default_voice_id: FALLBACK_VOICE, script_direction: ScriptDirection::Ltr },
    Language { code: LangCode::new("bg-BG"), native_name: "Български", english_name: "Bulgarian", tier: Tier::C, default_voice_id: FALLBACK_VOICE, script_direction: ScriptDirection::Ltr },
    Language { code: LangCode::new("ms-MY"), native_name: "Bahasa Melayu", english_name: "Malay", tier: Tier::C, default_voice_id: FALLBACK_VOICE, script_direction: ScriptDirection::Ltr },
    Language { code: LangCode::new("fil-PH"), native_name: "Filipino", english_name: "Tagalog", tier: Tier::C, default_voice_id: FALLBACK_VOICE, script_direction: ScriptDirection::Ltr },
];

pub fn all() -> &'static [Language] {
    LANGUAGES
}

pub fn by_code(code: &str) -> Option<&'static Language> {
    LANGUAGES.iter().find(|lang| lang.code.as_str() == code)
}

pub fn by_tier(tier: Tier) -> Vec<&'static Language> {
    LANGUAGES.iter().filter(|lang| lang.tier == tier).collect()
}
