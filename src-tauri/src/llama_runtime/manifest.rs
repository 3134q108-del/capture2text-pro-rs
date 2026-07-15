use serde::{Deserialize, Serialize};

#[allow(clippy::enum_variant_names)]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelId {
    #[serde(rename = "Qwen3Vl2bInstruct")]
    Qwen3Vl2bInstruct,
    #[serde(rename = "Qwen3Vl4bInstruct")]
    Qwen3Vl4bInstruct,
    #[serde(rename = "Qwen3Vl8bInstruct")]
    Qwen3Vl8bInstruct,
    #[serde(rename = "Qwen35_2b")]
    Qwen35_2b,
    #[serde(rename = "Qwen35_4b")]
    Qwen35_4b,
    #[serde(rename = "Qwen35_9b")]
    Qwen35_9b,
}

impl ModelId {
    pub fn supports_lang(&self, lang: &str) -> bool {
        self.spec().supported_lang_codes.contains(&lang)
    }

    pub fn is_qwen35(&self) -> bool {
        matches!(
            self,
            ModelId::Qwen35_2b | ModelId::Qwen35_4b | ModelId::Qwen35_9b
        )
    }

    pub fn spec(&self) -> &'static ModelSpec {
        match self {
            ModelId::Qwen3Vl2bInstruct => &QWEN3_VL_2B,
            ModelId::Qwen3Vl4bInstruct => &QWEN3_VL_4B,
            ModelId::Qwen3Vl8bInstruct => &QWEN3_VL_8B,
            ModelId::Qwen35_2b => &QWEN35_2B,
            ModelId::Qwen35_4b => &QWEN35_4B,
            ModelId::Qwen35_9b => &QWEN35_9B,
        }
    }

    #[allow(dead_code)]
    pub fn for_lang(lang: &str) -> ModelId {
        for &id in &ALL_MODELS {
            if id.supports_lang(lang) {
                return id;
            }
        }
        ModelId::Qwen3Vl8bInstruct
    }
}

#[allow(dead_code)]
pub struct ModelSpec {
    pub id: ModelId,
    pub display_name: &'static str,
    pub gguf_url: &'static str,
    pub mmproj_url: &'static str,
    pub chat_template: &'static str,
    pub ctx_size: u32,
    pub size_mb: u32,
    pub supported_lang_codes: &'static [&'static str],
}

impl ModelSpec {
    pub fn gguf_filename(&self) -> String {
        match self.id {
            ModelId::Qwen3Vl2bInstruct => "qwen3-vl-2b-instruct.Q4_K_M.gguf",
            ModelId::Qwen3Vl4bInstruct => "qwen3-vl-4b-instruct.Q4_K_M.gguf",
            ModelId::Qwen3Vl8bInstruct => "qwen3-vl-8b-instruct.Q4_K_M.gguf",
            ModelId::Qwen35_2b => "qwen3.5-2b.Q4_K_M.gguf",
            ModelId::Qwen35_4b => "qwen3.5-4b.Q4_K_M.gguf",
            ModelId::Qwen35_9b => "qwen3.5-9b.Q4_K_M.gguf",
        }
        .to_string()
    }

    pub fn mmproj_filename(&self) -> String {
        match self.id {
            ModelId::Qwen3Vl2bInstruct => "qwen3-vl-2b-instruct.mmproj.gguf",
            ModelId::Qwen3Vl4bInstruct => "qwen3-vl-4b-instruct.mmproj.gguf",
            ModelId::Qwen3Vl8bInstruct => "qwen3-vl-8b-instruct.mmproj.gguf",
            ModelId::Qwen35_2b => "qwen3.5-2b.mmproj.gguf",
            ModelId::Qwen35_4b => "qwen3.5-4b.mmproj.gguf",
            ModelId::Qwen35_9b => "qwen3.5-9b.mmproj.gguf",
        }
        .to_string()
    }
}

const QWEN3_VL_20_LANG_CODES: &[&str] = &[
    "zh-CN", "zh-TW", "en-US", "ja-JP", "ko-KR", "fr-FR", "de-DE", "es-ES", "pt-PT", "it-IT",
    "ru-RU", "id-ID", "tr-TR", "pl-PL", "vi-VN", "ar-SA", "th-TH", "hi-IN", "el-GR", "he-IL",
];

const QWEN3_VL_2B: ModelSpec = ModelSpec {
    id: ModelId::Qwen3Vl2bInstruct,
    display_name: "Qwen3-VL-2B-Instruct",
    gguf_url: "https://huggingface.co/unsloth/Qwen3-VL-2B-Instruct-GGUF/resolve/main/Qwen3-VL-2B-Instruct-Q4_K_M.gguf",
    mmproj_url: "https://huggingface.co/unsloth/Qwen3-VL-2B-Instruct-GGUF/resolve/main/mmproj-F16.gguf",
    chat_template: "chatml",
    ctx_size: 8192,
    size_mb: 1500,
    supported_lang_codes: &[
        "zh-CN", "zh-TW", "en-US", "ja-JP", "ko-KR", "fr-FR", "de-DE", "es-ES",
    ],
};

const QWEN3_VL_4B: ModelSpec = ModelSpec {
    id: ModelId::Qwen3Vl4bInstruct,
    display_name: "Qwen3-VL-4B-Instruct",
    gguf_url: "https://huggingface.co/unsloth/Qwen3-VL-4B-Instruct-GGUF/resolve/main/Qwen3-VL-4B-Instruct-Q4_K_M.gguf",
    mmproj_url: "https://huggingface.co/unsloth/Qwen3-VL-4B-Instruct-GGUF/resolve/main/mmproj-F16.gguf",
    chat_template: "chatml",
    ctx_size: 8192,
    size_mb: 2500,
    supported_lang_codes: &[
        "zh-CN", "zh-TW", "en-US", "ja-JP", "ko-KR", "fr-FR", "de-DE", "es-ES", "pt-PT", "it-IT",
        "ru-RU", "id-ID", "tr-TR", "pl-PL",
    ],
};

const QWEN3_VL_8B: ModelSpec = ModelSpec {
    id: ModelId::Qwen3Vl8bInstruct,
    display_name: "Qwen3-VL-8B-Instruct",
    gguf_url: "https://huggingface.co/unsloth/Qwen3-VL-8B-Instruct-GGUF/resolve/main/Qwen3-VL-8B-Instruct-Q4_K_M.gguf",
    mmproj_url: "https://huggingface.co/unsloth/Qwen3-VL-8B-Instruct-GGUF/resolve/main/mmproj-F16.gguf",
    chat_template: "chatml",
    ctx_size: 8192,
    size_mb: 5000,
    supported_lang_codes: QWEN3_VL_20_LANG_CODES,
};

const QWEN35_2B: ModelSpec = ModelSpec {
    id: ModelId::Qwen35_2b,
    display_name: "Qwen3.5-2B-Instruct",
    gguf_url: "https://huggingface.co/unsloth/Qwen3.5-2B-GGUF/resolve/main/Qwen3.5-2B-Q4_K_M.gguf",
    mmproj_url: "https://huggingface.co/unsloth/Qwen3.5-2B-GGUF/resolve/main/mmproj-F16.gguf",
    chat_template: "chatml",
    ctx_size: 8192,
    size_mb: 1900,
    supported_lang_codes: QWEN3_VL_20_LANG_CODES,
};

const QWEN35_4B: ModelSpec = ModelSpec {
    id: ModelId::Qwen35_4b,
    display_name: "Qwen3.5-4B-Instruct",
    gguf_url: "https://huggingface.co/unsloth/Qwen3.5-4B-GGUF/resolve/main/Qwen3.5-4B-Q4_K_M.gguf",
    mmproj_url: "https://huggingface.co/unsloth/Qwen3.5-4B-GGUF/resolve/main/mmproj-F16.gguf",
    chat_template: "chatml",
    ctx_size: 8192,
    size_mb: 3300,
    supported_lang_codes: QWEN3_VL_20_LANG_CODES,
};

const QWEN35_9B: ModelSpec = ModelSpec {
    id: ModelId::Qwen35_9b,
    display_name: "Qwen3.5-9B-Instruct",
    gguf_url: "https://huggingface.co/unsloth/Qwen3.5-9B-GGUF/resolve/main/Qwen3.5-9B-Q4_K_M.gguf",
    mmproj_url: "https://huggingface.co/unsloth/Qwen3.5-9B-GGUF/resolve/main/mmproj-F16.gguf",
    chat_template: "chatml",
    ctx_size: 8192,
    size_mb: 6300,
    supported_lang_codes: QWEN3_VL_20_LANG_CODES,
};

pub const ALL_MODELS: [ModelId; 6] = [
    ModelId::Qwen3Vl2bInstruct,
    ModelId::Qwen3Vl4bInstruct,
    ModelId::Qwen3Vl8bInstruct,
    ModelId::Qwen35_2b,
    ModelId::Qwen35_4b,
    ModelId::Qwen35_9b,
];

pub fn lookup(id: &ModelId) -> Option<&'static ModelSpec> {
    Some(id.spec())
}
