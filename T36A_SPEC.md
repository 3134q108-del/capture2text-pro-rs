# T36a · output_lang + TargetLang 擴 5 語（Rust only）

## 背景

User 要 5 種輸出語言：`zh-TW / zh-CN / en-US / ja-JP / ko-KR`。
目前只有 `zh / en` 二選一。這一步只擴後端，不動前端。

## 目標

擴 `output_lang` storage + `TargetLang` enum + 串接所有 match 點到 5 語。
默認值 `zh-TW`。

## 鎖死（MUST）

### 1. `src-tauri/src/output_lang.rs`

- `DEFAULT_LANG` 從 `"zh"` 改 `"zh-TW"`
- `sanitize(lang)` 擴：
  - 接受（case insensitive）：`zh-TW` / `zh-CN` / `en-US` / `ja-JP` / `ko-KR`
  - 輸出一律正規化成大小寫：`zh-TW` / `zh-CN` / `en-US` / `ja-JP` / `ko-KR`
  - 其他任何值 → 回 `DEFAULT_LANG`
  - **向後相容**：`"zh"` → `"zh-TW"`，`"en"` → `"en-US"`（讓舊 storage 檔自動升級）

### 2. `src-tauri/src/vlm/mod.rs`

TargetLang enum 從 2 變 5：
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetLang {
    TraditionalChinese,
    SimplifiedChinese,
    English,
    Japanese,
    Korean,
}
```

`display_name(self)` 擴：
- `TraditionalChinese => "繁體中文"`
- `SimplifiedChinese => "簡體中文"`
- `English => "英文"`（原本是 "English"，改統一中文）
- `Japanese => "日文"`
- `Korean => "韓文"`

**不改 build_system_prompt 的 format string 結構**（它已經用 display_name），只會自動吃到新 5 語。
其他用到 TargetLang 的地方（VlmJob 變體、try_submit_\*、ocr_and_translate、translate_text 等）**不動** — 它們只是 pass-through，不 match。

### 3. `src-tauri/src/capture/screenshot.rs`

`current_target_lang()` 從：
```rust
if output_lang::current() == "en" { TargetLang::English } else { TargetLang::Chinese }
```
改成 match 5 語：
```rust
fn current_target_lang() -> TargetLang {
    match output_lang::current().as_str() {
        "zh-CN" => TargetLang::SimplifiedChinese,
        "en-US" => TargetLang::English,
        "ja-JP" => TargetLang::Japanese,
        "ko-KR" => TargetLang::Korean,
        _ => TargetLang::TraditionalChinese, // 預設 + 向後相容
    }
}
```

### 4. `src-tauri/src/commands/translate.rs`

同樣的 match 邏輯擴 5 語（跟 screenshot.rs 同 pattern）。

### 5. `src-tauri/src/bin/vlm_smoke.rs`

CLI arg 擴 5 語：
```rust
let target_lang = match args[2].as_str() {
    "zh-TW" | "zh" => TargetLang::TraditionalChinese,
    "zh-CN" => TargetLang::SimplifiedChinese,
    "en-US" | "en" => TargetLang::English,
    "ja-JP" | "ja" => TargetLang::Japanese,
    "ko-KR" | "ko" => TargetLang::Korean,
    _ => return Err(io::Error::new(io::ErrorKind::InvalidInput,
        "language must be zh-TW / zh-CN / en-US / ja-JP / ko-KR")),
};
```

## 禁動

- **不動** `tray.rs`（T40 會做）
- **不動** 任何前端（T36b 會做）
- **不動** `window_state.rs`
- **不動** `scenarios.rs`、`tts` 模組
- **不動** `lib.rs`（commands 沒新增）

## 驗證

- `cargo check`（src-tauri/）
- 跑 `cargo build --bin vlm_smoke`（確保 CLI binary 編譯）
- **不需**跑 `npm build`（純 Rust）

## 風險點

1. **tray.rs 目前用 `== "zh"` / `== "en"`**：擴 output_lang 後這段會失效（checkbox 不會勾選）。這是**預期的**，T40 會處理。
   T36a 完成後 tray menu 兩個 checkbox 都會 unchecked，但不會 crash（sanitize 會回 "zh-TW"，tray 比 == "zh" 永遠 false，但 menu 仍能 render）。
2. **舊 storage 檔自動升級**：原本存 "zh" / "en" 的 `output_lang.txt` 下次讀會被 sanitize 升成 "zh-TW" / "en-US" 並立即 persist。
3. 現有 build_system_prompt format `"翻譯成{}的結果"` 仰賴 display_name。擴 5 語後新語言會是 `"翻譯成日文的結果"` 等，VLM 能理解。

## 回報

```
=== T36a 套改結果 ===
- 改 5 檔：
  - output_lang.rs: sanitize 擴 5 語 + default zh-TW
  - vlm/mod.rs: TargetLang 5 variant + display_name 對應
  - capture/screenshot.rs: current_target_lang match 擴
  - commands/translate.rs: retranslate match 擴
  - bin/vlm_smoke.rs: CLI match 擴
- cargo check: <結果>
- cargo build --bin vlm_smoke: <結果>

VERDICT: APPROVED
```

**直接套改，不需要先給 diff 提案**（純 enum 擴 + match 擴，機械性）。全部 UTF-8 NoBOM。
