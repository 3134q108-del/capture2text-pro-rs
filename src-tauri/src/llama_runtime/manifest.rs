use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelId {
    Qwen3Vl8bInstruct,
}

impl ModelId {
    pub fn supports_lang(&self, lang: &str) -> bool {
        match self {
            ModelId::Qwen3Vl8bInstruct => {
                matches!(
                    lang,
                    "zh-TW" | "zh-CN" | "en-US" | "ja-JP" | "ko-KR" | "de-DE" | "fr-FR"
                )
            }
        }
    }

    pub fn for_lang(lang: &str) -> ModelId {
        if ModelId::Qwen3Vl8bInstruct.supports_lang(lang) {
            ModelId::Qwen3Vl8bInstruct
        } else {
            ModelId::Qwen3Vl8bInstruct
        }
    }
}

pub struct ModelSpec {
    pub id: ModelId,
    pub gguf_url: &'static str,
    pub mmproj_url: &'static str,
    pub chat_template: &'static str,
    pub ctx_size: u32,
}

impl ModelSpec {
    pub fn gguf_filename(&self) -> &'static str {
        match self.id {
            ModelId::Qwen3Vl8bInstruct => "qwen3-vl-8b-instruct.Q4_K_M.gguf",
        }
    }

    pub fn mmproj_filename(&self) -> &'static str {
        match self.id {
            ModelId::Qwen3Vl8bInstruct => "qwen3-vl-8b-instruct.mmproj.gguf",
        }
    }
}

const QWEN3_VL_8B: ModelSpec = ModelSpec {
    id: ModelId::Qwen3Vl8bInstruct,
    gguf_url: "https://huggingface.co/unsloth/Qwen3-VL-8B-Instruct-GGUF/resolve/main/Qwen3-VL-8B-Instruct-Q4_K_M.gguf",
    mmproj_url: "https://huggingface.co/unsloth/Qwen3-VL-8B-Instruct-GGUF/resolve/main/mmproj-F16.gguf",
    chat_template: "chatml",
    ctx_size: 4096,
};

pub const ALL_MODELS: [ModelId; 1] = [ModelId::Qwen3Vl8bInstruct];

pub fn lookup(id: &ModelId) -> Option<&'static ModelSpec> {
    match id {
        ModelId::Qwen3Vl8bInstruct => Some(&QWEN3_VL_8B),
    }
}
