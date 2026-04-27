# T35j Edge TTS 並發被 RST 修復 + Partial 不清 speakingTarget

## Bug #1: 兩個 prefetch thread 並發打 Edge TTS 被 RST 導致超慢

### Log 證據（T35i 加的 timing log）
```
[tts-synth] runtime_ms=0     ← tokio runtime 非瓶頸
[tts-synth] client_ms=0      ← client init 非瓶頸
[tts-synth] synth_ms=33235 total_ms=33235 bytes=205344    ← 33 秒！
[tts-synth] synth_ms=32570 total_ms=32571 bytes=203184    ← 32 秒！
[tts-cache] prefetch failed ... websocket error: IO error:
  遠端主機已強制關閉一個現存的連線。 (os error 10054)
```

### Root cause
Microsoft Edge TTS 對同 IP 並發請求會 RST。我們 prefetch 原文 + 譯文並發 → RST → retry → 超慢 30+ 秒。

### Fix
`src-tauri/src/tts/mod.rs` 新增全局 Mutex 序列化 synthesize：

```rust
use std::sync::Mutex;

static TTS_SYNTH_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn synth_lock() -> &'static Mutex<()> {
    TTS_SYNTH_LOCK.get_or_init(|| Mutex::new(()))
}

pub fn synthesize_with_voice(text: &str, voice_code: &str) -> Result<Vec<u8>, TtsError> {
    if text.trim().is_empty() { return Err(TtsError::EmptyText); }
    let processed = preprocess_for_speech(text, voice_code);
    
    let _guard = synth_lock().lock().map_err(|_| TtsError::RequestFailed("synth lock poisoned".to_string()))?;
    eprintln!("[tts-synth] acquired lock voice={} text_len={}", voice_code, text.len());
    
    // 原本 timing log + runtime + client + synthesize 邏輯
    // ...
}
```

效果：兩個 prefetch thread 序列化合成，第二個等第一個完。總時間變成 "A_synth + B_synth"（~10+10=20 秒），但穩定、不會 RST retry 浪費。

## Bug #2: Retrans 播放中，譯文 partial 來把 speakingTarget 清掉

### Root cause
`src/result/ResultView.tsx` 的 `applyPartialPayload` 每次呼叫都 `setSpeakingTarget(null)`。
user 按 Retranslate 時播原文中 → 譯文 partial streaming 到 → setSpeakingTarget(null) → 按鈕從 Stop 變 Speak → user 按不到 Stop。

### Fix
移除 `applyPartialPayload` 裡的 `setSpeakingTarget(null)` 那行。
理由：partial 只是文字更新，不該 affect playback state。speakingTarget 應該由 tts-done listener 清（播完才清）。

`applySnapshot` 的 loading 分支同樣處理（移除 setSpeakingTarget(null) 或至少不在 Retrans source 時清）。

## 非目標
- 不動 Popup layout / Font / Settings / Tray
- 不改 EdgeTtsClient 共用策略（序列化已夠用）
- 不動 VLM 推理

## 驗收
- cargo check --all-targets 綠
- npm.cmd run build 綠
- 手動 restart dev：
  * Win+Q 截圖 -> Rust log 看到 `[tts-synth] acquired lock` 各兩次；兩個 synth 時間合理（~5-15 秒每個，sequential）
  * 按 Speak 原文播放中 -> 按 Retranslate -> 譯文合成中但 **Speak 原文 Stop 鍵保持可按**，按 Stop 立刻停
  * Speak 譯文 也測同邏輯

## 回報
Phase 1 diff + Phase 2 套改 + build（不碰 git）
CC commit: fix(tts): serialize edge tts synth to avoid RST + keep speakingTarget on partial (Stage 7a T35j)
