use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelId {
    Qwen3Vl4bInstruct,
}

impl ModelId {
    pub fn for_lang(lang: &str) -> ModelId {
        let _ = lang;
        ModelId::Qwen3Vl4bInstruct
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
            ModelId::Qwen3Vl4bInstruct => "qwen3-vl-4b-instruct.Q4_K_M.gguf",
        }
    }

    pub fn mmproj_filename(&self) -> &'static str {
        match self.id {
            ModelId::Qwen3Vl4bInstruct => "qwen3-vl-4b-instruct.mmproj.gguf",
        }
    }
}

const QWEN3_VL_4B: ModelSpec = ModelSpec {
    id: ModelId::Qwen3Vl4bInstruct,
    gguf_url: "https://huggingface.co/unsloth/Qwen3-VL-4B-Instruct-GGUF/resolve/main/Qwen3-VL-4B-Instruct-Q4_K_M.gguf",
    mmproj_url: "https://huggingface.co/unsloth/Qwen3-VL-4B-Instruct-GGUF/resolve/main/mmproj-F16.gguf",
    chat_template: "chatml",
    ctx_size: 4096,
};

pub const ALL_MODELS: [ModelId; 1] = [ModelId::Qwen3Vl4bInstruct];

pub fn lookup(id: &ModelId) -> Option<&'static ModelSpec> {
    match id {
        ModelId::Qwen3Vl4bInstruct => Some(&QWEN3_VL_4B),
    }
}
