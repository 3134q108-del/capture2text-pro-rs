# T37 修正：不要砍既有 tts::config API

## 問題

Codex 剛才的 edit 要砍掉一整堆既有 public fn，我擔心破壞 Stage 7a 剛穩定的 Popup TTS 流程。

## 修正策略

**保留所有既有 tts::config public API 不動**：
- `TtsConfig` struct
- `init_runtime()` / `get_config_runtime()` / `load_config()` / `save_config()`
- `set_active()` / `set_active_zh()` / `set_active_en()`
- `current_zh_voice()` / `current_en_voice()`
- `available_voices()`（不改名、不改簽名，但改內部實作）
- `default_config()` / `sanitize_config()`
- `storage_path()` / `write_config()` / `runtime_guard()`
- `TTS_RUNTIME` static
- `TtsVoiceOption` / `TtsRuntime` struct

## 只**新增**以下 item

1. 新 static `VOICE_CACHE: OnceLock<Mutex<Vec<TtsVoiceOption>>>`
2. 新 fn：
   - `pub fn fallback_voices() -> Vec<TtsVoiceOption>` 回硬 code 5 語清單
   - `pub fn fetch_remote_voices() -> Result<Vec<TtsVoiceOption>, String>` GET Edge endpoint parse
   - `pub fn cached_voices() -> Vec<TtsVoiceOption>` 讀 VOICE_CACHE；未初始化則 init fallback
   - `pub fn set_cached_voices(list: Vec<TtsVoiceOption>)` 寫 VOICE_CACHE
   - `pub fn init_voice_cache()` 嘗試 fetch，失敗 fallback，寫 cache
3. 改 **`available_voices()` 的實作** 成：
   ```rust
   pub fn available_voices() -> Vec<TtsVoiceOption> {
       cached_voices()
   }
   ```
   但 signature 完全不變，呼叫者無感。

## 分離新舊兩套

- **舊路徑（Popup TTS 流程）**：繼續用 TtsConfig（tts_config.json）+ current_zh_voice / current_en_voice / set_active_zh/en
- **新路徑（Settings Speech tab）**：全走 window_state.speech_voices HashMap + 新 set_speech_voice command

兩套共存。VOICE_CACHE 是共用的 voice clue pool，初始化一次供雙方查詢。

## 其他 T37 項目不變

- window_state.rs 擴 speech_* 6 欄位 + 6 setters
- commands/result_window.rs 新 6 set_speech_* commands
- tts/mod.rs 新 preview_voice / synthesize_full
- commands/tts.rs 新 preview_voice + refresh_tts_voices
- lib.rs setup + invoke_handler register
- SpeechTab.tsx 重寫
- SettingsView.css 加 class

## 動作

先給**修正後的套改計畫摘要**（每檔幾行改動），我 approve 後才動檔。
