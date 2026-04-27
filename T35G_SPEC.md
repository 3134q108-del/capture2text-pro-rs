T35e+T35f commit a72a261。下一顆 T35g TTS 深度重做。

[T35g] TTS 分 target + cache-ready 才能點 Speak + state 不再搶通道

### 現況問題（user 手測反映）
1. Speak 原文/譯文 共用同一 tts-done event 不帶 target → listener 無法區分哪邊播完，speakingTarget state 可能被錯誤清除
2. cache 未合成好按 Speak 會 fallback 即時合成（2-6 秒延遲）— user 不要這行為
3. 「原文跟譯文搶通道」= state 錯亂互相影響

### 修法目標
A. Rust speak / tts-done event 帶 target 辨識
B. React cache-ready 控制按鈕 disabled（未合成 = 按鈕 disabled + 標「合成中…」）
C. cache miss 不 fallback 即時合成（回錯誤讓 user 知道要等）
D. tts-done 只清對應 target 的 speakingTarget

### 範圍

#### Rust: src-tauri/src/commands/tts.rs

```rust
#[tauri::command]
pub fn speak(app: AppHandle, target: String, text: String, lang: String) -> Result<(), String> {
    // target = "original" | "translated"
    if text.trim().is_empty() { return Ok(()); }
    let voice_code = tts::current_voice_for_lang(lang.as_str());
    
    // cache-only: miss 直接回錯誤，不 fallback synthesize
    let Some(mp3) = tts::cache_get(&text, &voice_code) else {
        eprintln!("[tts] cache miss voice={} — refusing to synthesize, caller should wait prefetch", voice_code);
        return Err("not-ready".to_string());
    };
    
    std::thread::spawn(move || {
        if let Err(err) = tts::play_mp3(&mp3) {
            eprintln!("[tts] play failed: {}", err);
        }
        // tts-done 帶 target payload
        let _ = app.emit("tts-done", serde_json::json!({ "target": target }));
    });
    Ok(())
}

// 新 command: 查 cache 是否已有該文字的 MP3
#[tauri::command]
pub fn is_tts_cached(text: String, lang: String) -> bool {
    if text.trim().is_empty() { return false; }
    let voice_code = tts::current_voice_for_lang(lang.as_str());
    tts::cache_get(&text, &voice_code).is_some()
}
```

#### Rust: src-tauri/src/vlm/mod.rs — prefetch 完成時 emit event

```rust
// emit_vlm_event 裡 status=success 的 prefetch spawn 改為：
std::thread::spawn({
    let app = app_handle.clone();
    let original = original.clone();
    move || {
        if !original.trim().is_empty() {
            let voice = tts::current_voice_for_lang(if contains_chinese(&original) { "zh" } else { "en" });
            tts::prefetch(&original, &voice);
            let _ = app.emit("tts-prefetch-done", serde_json::json!({ "target": "original" }));
        }
    }
});
// 同樣對 translated
```

#### Rust: src-tauri/src/lib.rs
- invoke_handler 加 `is_tts_cached`

#### React: src/result/ResultView.tsx

新 state:
```tsx
const [cacheReady, setCacheReady] = useState<{ original: boolean; translated: boolean }>({ original: false, translated: false });
```

VLM success clear cacheReady:
- applyFinalPayload 裡 status=success 時 setCacheReady({ original: false, translated: false })（等 prefetch-done event 設 true）
- applySnapshot 裡 snapshot.status=success 時同上

新 listener:
```tsx
const offPrefetchDone = await listen("tts-prefetch-done", (event) => {
  const target = event.payload?.target;
  if (target === "original" || target === "translated") {
    setCacheReady(prev => ({ ...prev, [target]: true }));
  }
});
// cleanup 時 offPrefetchDone?.()
```

改現有 tts-done listener:
```tsx
const offTtsDone = await listen("tts-done", (event) => {
  const target = event.payload?.target;
  // 只清對應 target 的 state
  setSpeakingTarget(prev => (prev === target ? null : prev));
});
```

改 toggleSpeak:
```tsx
async function toggleSpeak(target: "original" | "translated") {
  if (speakingTarget === target) {
    // stop 自己
    try { await invoke("stop_speaking"); } catch {}
    // state 由 tts-done 清（或保險手動清）
    setSpeakingTarget(null);
    return;
  }
  if (speakingTarget !== null) {
    // 有別的在播 — 先停對方（按鈕 disabled 不該點到這）
    try { await invoke("stop_speaking"); } catch {}
  }
  const text = target === "original" ? original : translated;
  if (!text.trim()) return;
  const lang = detectLang(text);
  setSpeakingTarget(target);
  try {
    await invoke("speak", { target, text, lang });
  } catch (err) {
    console.warn("[speak] failed", err);
    setSpeakingTarget(null);
  }
}
```

按鈕 disabled 條件:
```tsx
// Speak 原文 按鈕
disabled={
  !original.trim()
  || !cacheReady.original  // 未合成好
  || (speakingTarget !== null && speakingTarget !== "original")  // 別的在播
}
// label: 未 ready 時顯示「合成中…」，否則正常 Speak/Stop
{!cacheReady.original && original.trim() ? "合成中…" 
 : speakingTarget === "original" ? "Stop"
 : "Speak 原文"}
```

同樣對 Speak 譯文。

#### React mount 初次：
useEffect 註冊 listeners 後，invoke is_tts_cached 對原文 + 譯文各查一次（catch-up case），若已 cached 設 cacheReady。

### 非目標
- 不改 Popup layout / Font / Settings / Tray
- 不動 window rebuild (T35e)
- 不動 VLM 推理流程

### 驗收
- cargo check --all-targets 綠
- npm.cmd run build 綠
- 手動 restart dev：
  * Win+Q 截圖 → VLM 跑完彈窗顯示原文+譯文
  * 瞬間按 Speak 原文 → 若 prefetch 未好按鈕應 disabled 標「合成中…」
  * 等 1-3 秒 prefetch 完 → 按鈕 enabled
  * 按 Speak 原文 → 立刻播出 + 按鈕變 Stop
  * 播原文中按 Speak 譯文 → 原文停、譯文播、只有譯文按鈕 Stop state
  * 按 Stop → 停、按鈕回 Speak 譯文
  * Speak 原文 +Speak 譯文 state 不會互相錯亂

回報：Phase 1 提案 + Phase 2 套改 + build（不碰 git）
CC commit 訊息：refactor(tts): per-target speak state + cache-ready gating (Stage 7a T35g)

Phase 1 開始。
