use serde::{Deserialize, Serialize};

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ModelId {
    Qwen3Vl2bInstruct,
    Qwen3Vl4bInstruct,
    Qwen3Vl8bInstruct,
}

impl ModelId {
    pub fn supports_lang(&self, lang: &str) -> bool {
        self.spec().supported_lang_codes.contains(&lang)
    }

    pub fn spec(&self) -> &'static ModelSpec {
        match self {
            ModelId::Qwen3Vl2bInstruct => &QWEN3_VL_2B,
            ModelId::Qwen3Vl4bInstruct => &QWEN3_VL_4B,
            ModelId::Qwen3Vl8bInstruct => &QWEN3_VL_8B,
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
        }
        .to_string()
    }

    pub fn mmproj_filename(&self) -> String {
        match self.id {
            ModelId::Qwen3Vl2bInstruct => "qwen3-vl-2b-instruct.mmproj.gguf",
            ModelId::Qwen3Vl4bInstruct => "qwen3-vl-4b-instruct.mmproj.gguf",
            ModelId::Qwen3Vl8bInstruct => "qwen3-vl-8b-instruct.mmproj.gguf",
        }
        .to_string()
    }
}

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
    supported_lang_codes: &[
        "zh-CN", "zh-TW", "en-US", "ja-JP", "ko-KR", "fr-FR", "de-DE", "es-ES", "pt-PT", "it-IT",
        "ru-RU", "id-ID", "tr-TR", "pl-PL", "vi-VN", "ar-SA", "th-TH", "hi-IN", "el-GR", "he-IL",
    ],
};

pub const ALL_MODELS: [ModelId; 3] = [
    ModelId::Qwen3Vl2bInstruct,
    ModelId::Qwen3Vl4bInstruct,
    ModelId::Qwen3Vl8bInstruct,
];

pub fn lookup(id: &ModelId) -> Option<&'static ModelSpec> {
    Some(id.spec())
}
