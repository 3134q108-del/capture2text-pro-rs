use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelId {
    Qwen3Vl8bInstruct,
    Pixtral12b2409,
}

impl ModelId {
    pub fn supports_lang(&self, lang: &str) -> bool {
        match self {
            ModelId::Qwen3Vl8bInstruct => {
                matches!(lang, "zh-TW" | "zh-CN" | "en-US" | "ja-JP" | "ko-KR")
            }
            ModelId::Pixtral12b2409 => matches!(lang, "de-DE" | "fr-FR"),
        }
    }

    pub fn for_lang(lang: &str) -> ModelId {
        if ModelId::Pixtral12b2409.supports_lang(lang) {
            ModelId::Pixtral12b2409
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
            ModelId::Qwen3Vl8bInstruct => "qwen3-vl-4b-instruct.Q4_K_M.gguf",
            ModelId::Pixtral12b2409 => "pixtral-12b-2409.Q4_K_M.gguf",
        }
    }

    pub fn mmproj_filename(&self) -> &'static str {
        match self.id {
            ModelId::Qwen3Vl8bInstruct => "qwen3-vl-4b-instruct.mmproj.gguf",
            ModelId::Pixtral12b2409 => "pixtral-12b-2409.mmproj.gguf",
        }
    }
}

const QWEN3_VL_8B: ModelSpec = ModelSpec {
    id: ModelId::Qwen3Vl8bInstruct,
    gguf_url: "https://huggingface.co/unsloth/Qwen3-VL-4B-Instruct-GGUF/resolve/main/Qwen3-VL-4B-Instruct-Q4_K_M.gguf",
    mmproj_url: "https://huggingface.co/unsloth/Qwen3-VL-4B-Instruct-GGUF/resolve/main/mmproj-F16.gguf",
    chat_template: "chatml",
    ctx_size: 4096,
};

const PIXTRAL_12B: ModelSpec = ModelSpec {
    id: ModelId::Pixtral12b2409,
    gguf_url: "https://huggingface.co/ggml-org/pixtral-12b-GGUF/resolve/main/pixtral-12b-Q4_K_M.gguf",
    mmproj_url: "https://huggingface.co/ggml-org/pixtral-12b-GGUF/resolve/main/pixtral-12b-2409.mmproj.gguf",
    chat_template: "pixtral",
    ctx_size: 4096,
};

pub const ALL_MODELS: [ModelId; 2] = [ModelId::Qwen3Vl8bInstruct, ModelId::Pixtral12b2409];

pub fn lookup(id: &ModelId) -> Option<&'static ModelSpec> {
    match id {
        ModelId::Qwen3Vl8bInstruct => Some(&QWEN3_VL_8B),
        ModelId::Pixtral12b2409 => Some(&PIXTRAL_12B),
    }
}
