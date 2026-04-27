# T35h TTS 合成前字串預處理

## 需求（user 反映）
- `>` / `<` 要念「大於」「小於」（中文）或 "greater than" / "less than"（英文）
- `"` `*` `_` `` ` `` `~` `#` 等 markdown 符號不要念出來（刪除）

## 現況
Rust `tts::synthesize_with_voice(text, voice_code)` 直接把 text 送 Edge TTS。符號沒清理 → Edge TTS 某些 voice 會把 `*` 念 "star"、`_` 念 "underscore"，也可能對 SSML 造成 issue。

## 實作

### src-tauri/src/tts/mod.rs 新 helper
新增 `pub fn preprocess_for_speech(text: &str, voice_code: &str) -> String`：

1. 從 voice_code 前綴判斷語言：
   - `zh-` 開頭 -> 用中文規則
   - 其他 -> 用英文規則

2. 規則（**順序重要：longer pattern 先處理**）：
   - Multi-char：`>=` / `<=` / `!=` / `==`
   - Single-char：`>` / `<`
   - 刪除（共用）：`*` `_` `` ` `` `#` `"`（替換成空字串）

3. 中文規則：
   - `>=` -> `大於等於`
   - `<=` -> `小於等於`
   - `!=` -> `不等於`
   - `==` -> `等於`
   - `>` -> `大於`
   - `<` -> `小於`

4. 英文規則：
   - `>=` -> ` greater than or equal to `
   - `<=` -> ` less than or equal to `
   - `!=` -> ` not equal to `
   - `==` -> ` equals `
   - `>` -> ` greater than `
   - `<` -> ` less than `

5. 刪除規則（兩語共用）：全部替換 `*` `_` `` ` `` `#` `"` 為 `""`

### synthesize_with_voice 改用 preprocess 後的文字
在 `synthesize_with_voice(text, voice_code)` 頂端：
```
let processed = preprocess_for_speech(text, voice_code);
// 接下來用 processed 送 Edge TTS API
```

**注意**：cache key 保留原 text，不用 processed text（cache 以 user 看到的 text 為 key）。

### prefetch 也要改
`prefetch(text, voice_code)` 裡合成前同樣走 preprocess（因為合成實際是 synthesize_with_voice 負責），所以改 synthesize_with_voice 就夠了，prefetch 不用動。

## 驗收
- cargo check --all-targets 綠
- npm.cmd run build 綠
- 手動 restart dev：
  * OCR 一段含 `*`, `"`, `>` 的文字（例如 Markdown `**bold**` 或 `x > 5`）
  * 按 Speak 原文 -> 聽到 `*` 沒被念、`>` 念成「大於」
  * 中文譯文若含 `"` -> 沒被念出來
  * 英文原文若含 `>` -> 念 "greater than"

## 非目標
- 不改 Popup UI
- 不做 Settings Speech tab（T37 做 UI 編輯）
- 不動 cache 結構（cache key 保持原 text）
- 不加其他規則（user 後續提再加）

## 回報
Phase 1 diff + Phase 2 套改 + build（不碰 git）
CC commit 訊息：feat(tts): preprocess special chars before Edge TTS synthesis (Stage 7a T35h)

---

## T35h 追加需求：Retranslate 不重 prefetch 原文

### 現況 bug（user 反映）
按 Retranslate 後，Speak 原文按鈕也變「合成中…」— 但原文沒變，原文 MP3 已在 cache 不需重來。

### 修法

**Rust `src-tauri/src/vlm/mod.rs` 的 emit_vlm_event 裡**（status=success spawn prefetch thread 的地方）：
檢查 payload.source 欄位：
- 若 source == "Retrans" -> **只 prefetch 譯文**，不 prefetch 原文（也不 emit tts-prefetch-done{original}）
- 其他（Win+Q/W/E） -> 照舊兩個都 prefetch

**React `src/result/ResultView.tsx` 的 applyFinalPayload**：
- 若 p.source === "Retrans" -> setCacheReady(prev => ({ ...prev, translated: false }))  // 只清譯文
- 其他 -> setCacheReady({ original: false, translated: false })  // 兩個都清

applySnapshot 也同樣邏輯判斷。

### 驗收（追加）
- 按 Retranslate -> Speak 原文按鈕**不變「合成中…」**（保持可按）；Speak 譯文短暫「合成中…」後恢復
- Win+Q/W/E 走新截圖 -> 兩個按鈕都會先「合成中…」正常流程

---

## 再追加：`~` 不在刪除列表（user 意見）

user 說 `~` 作為語氣詞（例 `到~`）不適合硬刪，但要 TTS 念「波浪號」也怪。

**決定**：`~` **不進刪除列表**，交給 Edge TTS 預設處理（通常 Edge TTS 會 ignore `~` 不念）。
- 最終刪除列表：`*` `_` `` ` `` `#` `"`（**不含 `~`**）
- 未來 T37 Settings Speech tab 讓 user 自訂規則時可加

其他規則（`>` `<` 替換成「大於」「小於」等）不變。
