# T50b · Pixtral-12B 切換 + output_lang 擴 7 語

## 背景

T50a 完成 llama.cpp runtime + Qwen3-VL-4B（變數名還是 Qwen3Vl8bInstruct 技術債）當預設 VLM 處理 zh/en/ja/ko/zh-CN。

T50b：擴 output_lang 從 5 語到 **7 語**（加 `de-DE` / `fr-FR`），德/法時自動 switch 到 Pixtral-12B（已下載過）。

## 鎖死（MUST）

### 1. `src-tauri/src/output_lang.rs`：擴 7 語

`DEFAULT_LANG` 保持 `"zh-TW"`。

`sanitize(lang)` 擴：
- 正規化（case insensitive）接受：`zh-TW` / `zh-CN` / `en-US` / `ja-JP` / `ko-KR` / **`de-DE`** / **`fr-FR`**
- 向後相容舊值：`zh` → `zh-TW`，`en` → `en-US`，`ja`→`ja-JP`，`ko`→`ko-KR`，`de`→`de-DE`，`fr`→`fr-FR`
- 其他值 → `DEFAULT_LANG`

### 2. `src-tauri/src/vlm/mod.rs`：TargetLang 擴 7 variant

```rust
pub enum TargetLang {
    TraditionalChinese,
    SimplifiedChinese,
    English,
    Japanese,
    Korean,
    German,      // 新
    French,      // 新
}

// display_name
Self::German => "德文",
Self::French => "法文",
```

build_system_prompt 自動吃新 display_name。

### 3. `src-tauri/src/capture/screenshot.rs::current_target_lang()` + `src-tauri/src/commands/translate.rs`：match 擴

```rust
match output_lang::current().as_str() {
    "zh-CN" => TargetLang::SimplifiedChinese,
    "en-US" => TargetLang::English,
    "ja-JP" => TargetLang::Japanese,
    "ko-KR" => TargetLang::Korean,
    "de-DE" => TargetLang::German,      // 新
    "fr-FR" => TargetLang::French,      // 新
    _ => TargetLang::TraditionalChinese,
}
```

### 4. `src-tauri/src/bin/vlm_smoke.rs`：CLI 擴

```rust
"de-DE" | "de" => TargetLang::German,
"fr-FR" | "fr" => TargetLang::French,
```

error message 更新。

### 5. `src-tauri/src/llama_runtime/manifest.rs`：加 ModelId::Pixtral12b 的 lang set

新增 helper：
```rust
impl ModelId {
    /// 哪個 model 負責哪些語言
    pub fn supports_lang(&self, lang: &str) -> bool {
        match self {
            ModelId::Qwen3Vl8bInstruct => matches!(lang, "zh-TW" | "zh-CN" | "en-US" | "ja-JP" | "ko-KR"),
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
```

### 6. `src-tauri/src/llama_runtime/mod.rs`：switch_model 邏輯

新增 public fn `ensure_model_for_lang(lang: &str)`：
```rust
pub fn ensure_model_for_lang(lang: &str) -> Result<(), String> {
    let target = manifest::ModelId::for_lang(lang);
    if active_model().as_ref() == Some(&target) {
        return Ok(());
    }
    eprintln!("[llama-runtime] switching model for lang={} target={:?}", lang, target);
    switch_model(target)  // 已在 T50a 實作；內部 supervisor::stop + spawn_for + set_active_model
}
```

### 7. 在 VLM request 前觸發 switch

`vlm/mod.rs` 的 worker loop 處理 `VlmJob::OcrAndTranslate` / `TranslateText` 時，在 inference 呼叫前加：
```rust
let lang_code = target_lang_to_code(target_lang);  // "de-DE" 等
if let Err(err) = crate::llama_runtime::ensure_model_for_lang(&lang_code) {
    // emit error event
    return;
}
```

新 helper `target_lang_to_code`:
```rust
fn target_lang_to_code(lang: TargetLang) -> &'static str {
    match lang {
        TargetLang::TraditionalChinese => "zh-TW",
        TargetLang::SimplifiedChinese => "zh-CN",
        TargetLang::English => "en-US",
        TargetLang::Japanese => "ja-JP",
        TargetLang::Korean => "ko-KR",
        TargetLang::German => "de-DE",
        TargetLang::French => "fr-FR",
    }
}
```

switch 會觸發 kill 舊 llama-server + spawn 新的，約 **15-20 秒**。user 已接受此延遲。

### 8. `src-tauri/src/tray.rs`：輸出語言 submenu 擴 7

加 2 個 CheckMenuItem：`lang_de_de` / `lang_fr_fr`（label「德文」/「法文」），handler 同 pattern。

window-state-changed 和 output-language-changed listener 擴：從 5 變成 7 個 set_checked 互斥更新。

### 9. `src/settings/tabs/TranslateTab.tsx`：Output Language radio 擴 7

`LANG_OPTIONS` 加兩筆：
```tsx
{ code: "de-DE", label: "德文" },
{ code: "fr-FR", label: "法文" },
```

type `OutputLang` 擴：`"zh-TW" | "zh-CN" | "en-US" | "ja-JP" | "ko-KR" | "de-DE" | "fr-FR"`。

normalizeLang 擴新 code。

### 10. 確認模型已存在（bootstrap 不重複下載）

T50a 已下 Pixtral 檔案於 `%LOCALAPPDATA%\Capture2TextPro\models\pixtral-12b-2409.Q4_K_M.gguf` + mmproj。`llama_runtime::ensure_model_installed` 會檢查存在（並走 T50A_FIX6 的 size verify），pass 就直接 spawn。

### 11. Pixtral chat_template 在 manifest 應是 `"pixtral"` — 確認 T50a 實作正確

## 禁動

- **不動** VLM client（OpenAI API streaming）邏輯
- **不動** TTS / Speak / clipboard 邏輯
- **不動** ollama_boot.rs（T50c 才刪）

## 驗證

- `cargo check` + `cargo build` 通過
- `npm build` 通過
- UTF-8 NoBOM

## 回報

```
=== T50b 套改結果 ===
- output_lang.rs sanitize 擴 7
- vlm/mod.rs TargetLang 7 variant
- screenshot.rs / translate.rs / vlm_smoke.rs match 擴
- llama_runtime/manifest.rs 加 for_lang
- llama_runtime/mod.rs ensure_model_for_lang
- vlm/mod.rs worker 處理時 ensure_model_for_lang
- tray.rs 輸出語言擴 7 + listener 更新
- TranslateTab.tsx LANG_OPTIONS 7
- cargo check/build/npm build: <結果>
- UTF-8 NoBOM

VERDICT: APPROVED
```

**直接套改**。UTF-8 NoBOM。
