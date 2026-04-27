# Stage 7 UI 重構 · 定案版（user 逐頁討論後）

## Context

Tauri 2 rewrite Stage 1-6 完成。User 手測 7a 發現 **bug #3 彈窗空白**（VLM log 有原文+譯文但 React 沒收到 event）— blocker，必須先修。然後 user 逐頁討論原版 11 個 UI 頁面，決定出最終精簡結構。

---

## Stage 7 最終結構（user 拍板）

### Popup 翻譯彈窗（ResultView）
```
┌─ [原生 title bar]  Capture2Text - OCR Text        — ☐ × ┐
│                                                           │
│  [原文 textarea 可編輯]                                    │
│                                                           │
│  [譯文 textarea 唯讀，空時隱藏]                            │
│                                                           │
│  ☑ Topmost   Font...  Retranslate  Speak  Copy原文  Copy譯文  OK │
└───────────────────────────────────────────────────────────┘
```

- Speak 按鈕：若原文選了範圍 → 只播選取；否則播全部
- Font 按鈕：開系統字型選擇 + 套兩 textarea + 存 window_state.popup_font
- OK：依 clipboard flags 寫 clipboard + 關窗
- 尺寸/位置關窗存檔、下次還原

### Tray 系統選單
```
Settings...
─────────────
Show Popup Window    [✓]
Save to Clipboard    [✓]
─────────────
Output Language  ▸   ⦿ 繁體中文
                     ⦾ 簡體中文
                     ⦾ 英文
                     ⦾ 日文
                     ⦾ 韓文
─────────────
About...
Exit
```

### Settings 視窗（4 tabs，原版 9 tabs 精簡）
左側 nav list、右側 stacked content、底部 OK/Cancel

**Tab 1 · Translate**
- Output Language radio（5 語，跟 tray 同源）
- ☑ Append translation to clipboard
- Separator dropdown（6 選項對齊原版：Space / Tab / Line Break / Comma / Semicolon / Pipe）
- Scenarios 管理（通用/輪機/遊戲/程式/醫療 + 自訂，prompt 編輯 + CRUD）

**Tab 2 · Speech**
- ☐ Enable Text-to-speech
- Volume slider (0-100)
- Rate slider (-50~+50)
- Pitch slider (-50~+50)
- Voice per output language（5 語各一列）：
  - 每列：[voice dropdown ▼]  「sample 文字 input」  [▶ 試聽]
  - Voice list：**啟動時動態抓 Edge TTS voice list API**，失敗 fallback 寫死清單
  - Sample 文字：每語預設一句，user 可編
- 試聽：Rust 合成 + rodio 播（同 pipeline）

**Tab 3 · Output**
- ☑ Save to clipboard（跟 tray 同源）
- ☑ Show popup window（跟 tray 同源）
- ☐ Log captures to file（簡化版：固定模板 `時戳\t原文\t譯文\n`，只給 checkbox + 檔案位置）

**Tab 4 · About**
- App 名稱 + 版本（`getVersion()`）
- OCR+翻譯模型：`qwen3-vl:8b-instruct` + Ollama endpoint + [檢查連線]
- 快捷鍵區塊：Win+Q 區域 / Win+W 視窗 / Win+E 全螢幕
- TTS 引擎：Microsoft Edge TTS
- 原版資訊：Christopher Brochtrup + GPL-3
- GitHub links：原版 repo + 我們 fork（兩個都放）
- 檢查更新 notice（啟動時 fetch GitHub releases API 比版本）
- [匯出設定] [匯入設定] 按鈕（複製 `%LOCALAPPDATA%/Capture2TextPro/` ↔ user 選的資料夾）

### 捨棄的原版 tab（VLM 架構不適用）
- OCR Language（VLM 自動偵測，user 已拒 Input Language hint）
- OCR Options（Tesseract 行偵測像素參數）
- Capture Box（顏色寫死紅 25% alpha 對齊原版預設）
- Preview（VLM 一次性推理，無即時 preview 概念）
- Replace（Scenarios prompt 替代更強）
- Hotkeys（三鍵寫死資訊塞進 About，v0.2 再做可編輯）

---

## 存檔機制

```
%LOCALAPPDATA%/Capture2TextPro/
├── scenarios.json          情境 prompt 列表（T22 已做）
├── output_lang.txt         當前輸出語言（T24 已做）
└── window_state.json       所有其他 settings
```

**window_state.json 完整 schema**：
```jsonc
{
  // Popup 幾何（T26 已做）
  "popup_width": 661,
  "popup_height": 371,
  "popup_x": null,
  "popup_y": null,
  "popup_topmost": true,
  "popup_font": null,              // { family, size_pt }
  "popup_show_enabled": true,

  // Clipboard（T26 schema 有，T32 wire up）
  "save_to_clipboard": true,
  "translate_append_to_clipboard": false,
  "translate_separator": "Space",  // Space/Tab/LineBreak/Comma/Semicolon/Pipe

  // Output（T38 新增）
  "log_enabled": false,
  "log_file_path": "%LOCALAPPDATA%/Capture2TextPro/captures.log",

  // Speech（T37 新增）
  "speech_enabled": false,
  "speech_volume": 70,
  "speech_rate": 0,
  "speech_pitch": 0,
  "speech_voices": {
    "zh-TW": "zh-TW-HsiaoChenNeural",
    "zh-CN": "zh-CN-XiaoxiaoNeural",
    "en-US": "en-US-AvaNeural",
    "ja-JP": "ja-JP-NanamiNeural",
    "ko-KR": "ko-KR-SunHiNeural"
  },
  "speech_samples": {
    "zh-TW": "歡迎使用翻譯助理，這是聲音試聽",
    "zh-CN": "欢迎使用翻译助理，这是声音试听",
    "en-US": "Hello, this is a voice preview.",
    "ja-JP": "こんにちは、音声のプレビューです。",
    "ko-KR": "안녕하세요, 음성 미리듣기입니다."
  }
}
```

---

## Task 清單（重整）

### 已完成（Stage 7a）
- ✅ **T25** VLM state latch（`aec627b`）
- ✅ **T26** decorations:true + window_state + geometry 持久化（`aa7cb63`）
- ✅ **T27** ResultView layout + 淺色主題（`da38a67`）

### Blocker · 優先修
- 🚨 **T34** Bug fix · 彈窗原文/譯文空白 + 加診斷 log
  - 現況：Rust log 有 original + translated（VLM success），但 React listen callback 沒 fire
  - 疑點：StrictMode double-mount race / emit_to target 是否送到正確 window
  - 方法：加 Rust eprintln + React console.log instrumentation，user 測後看 log 決定 fix 方向
  - 備案：若 StrictMode race → 移除 `<StrictMode>` 或改 setup 邏輯

### Popup 剩餘（Stage 7a 尾）
- **T27.5** Font picker + 持久化 `window_state.popup_font`
- **T35** Popup 補功能：
  - Speak 按鈕支援選取範圍（原文 textarea 有 selection 則只播選取）
  - Copy 譯文按鈕新增（現有 Copy 拆成 Copy 原文 + Copy 譯文）

### Settings 重做（Stage 7b）
- **T28** Settings 4-tab shell
  - 重寫 SettingsView.tsx：左 nav list + 右 stacked + 底 OK/Cancel
  - 4 個空 tab 檔：`TranslateTab.tsx` / `SpeechTab.tsx` / `OutputTab.tsx` / `AboutTab.tsx`
  - 新 Rust commands skeleton：get/set Output Language / get/set window_state fields
  - 淺色主題對齊 T27 tokens
- **T36** Translate tab 內容：Output Language radio / Append translation / Separator / Scenarios editor（從舊 SettingsView 搬）
- **T37** Speech tab 內容（最大顆）：
  - Enable / Volume / Rate / Pitch slider
  - Edge TTS voice list **動態抓** + fallback static list
  - 5 語 voice dropdown + 可編 sample text + 試聽按鈕
  - Rust 新 command `preview_voice(voice_code, text)` 合成+播放
  - TTS speak 實際套用 rate/pitch/volume
- **T38** Output tab 內容：Save to clipboard / Show popup / Log to file 簡化版 + log 寫入邏輯
- **T39** About tab 內容：版本 / 模型資訊 / Ollama 連線檢查 / 快捷鍵顯示 / GitHub × 2 / 檢查更新 / 匯出匯入
- **T30** OK/Cancel footer + Scenarios dirty-state + 健康檢查 Retry

### Tray + Clipboard + 整合（Stage 7c）
- **T40** Tray 重寫對齊新結構：
  - `Settings... / — / Show Popup(☑) / Save to Clipboard(☑) / — / Output Language 5語 / — / About / Exit`
  - `About` emit `settings-navigate:about` + show settings
  - `Show Popup off` → `ensure_result_window_visible` 跳過 show
- **T32** Clipboard Rust arboard pipeline：
  - `src-tauri/src/clipboard.rs` 新檔，`write_text(&str)`
  - VLM success 時依 state 決定寫啥 + Popup OK 也呼叫
  - Separator 6 選項實作
- **T41** `window-state-changed` event 跨視窗同步（tray ↔ settings ↔ popup）

### 收尾
- **T42** 整體手測 + memory 更新

---

## Codex 協作模式
- 每 task 走 五欄 spec（目標/範圍/驗收/非目標/回報）
- Phase 1: diff 提案 → CC review
- Phase 2: 套改 + cargo check + npm.cmd run build
- **CC 端 commit**（.git 權限 MSYS2 uid 專屬）
- Bug #3 修完 restart dev，user 手測才進下一 task

---

## 關鍵檔案

### Rust 新增/修改
- `vlm/state.rs`（T25 已做）
- `window_state.rs`（T26 已做，擴 schema）
- `clipboard.rs`（T32 新）
- `commands/result_window.rs`（擴：`preview_voice` / `export_settings` / `import_settings` / `check_ollama_health` / `open_github` / `check_for_updates`）
- `commands/output_lang.rs`（T28 新）
- `commands/window_state.rs`（T28 新，各 setter）
- `tts/mod.rs`（T37 擴：動態 voice list + rate/pitch/volume）
- `capture/log.rs`（T38 新：寫 OCR log 檔）
- `tray.rs`（T40 重寫）
- `lib.rs`（串起所有新 commands）

### React 新增/修改
- `result/ResultView.tsx`（T34 debug / T27.5 Font / T35 Copy 拆分 + Speak 選取）
- `settings/SettingsView.tsx`（T28 重寫 shell）
- `settings/tabs/TranslateTab.tsx`（T36）
- `settings/tabs/SpeechTab.tsx`（T37）
- `settings/tabs/OutputTab.tsx`（T38）
- `settings/tabs/AboutTab.tsx`（T39）

---

## 驗收

### T34 Bug fix
- Win+Q 截圖 → 彈窗**正確顯示原文+譯文**（不再空白）
- DevTools console 有診斷 log

### Stage 7a 尾（T27.5 + T35）
- 點 Font 按鈕開系統字型選擇、套用到兩 textarea、關窗還原
- 原文選一段按 Speak 只播選取；沒選播全部
- Copy 原文 / Copy 譯文 各自可用

### Stage 7b（T28+T36+T37+T38+T39+T30）
- Settings 4 tab 可切、右邊內容對應
- Translate tab：切 Output Language 即時反映；勾 Append + 選 Separator → Clipboard 依規則組
- Speech tab：Edge TTS voice list 能抓到（>5 每語）；每語 dropdown 選 voice + 試聽有聲
- Output tab：勾 Log → `captures.log` 寫入成功
- About tab：Ollama 連線檢查 / 匯出 backup / 匯入還原

### Stage 7c（T40+T32+T41）
- Tray 選單對齊新結構
- 勾 Save + 勾 Append + Separator=LineBreak → OCR 完 Ctrl+V 有 `原文\n譯文`
- tray 切 Output Language → settings 的 radio 即時反映
