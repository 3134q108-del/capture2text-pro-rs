# T35i Retrans cacheReady partial path fix + synth timing log

## Bug #1: Retranslate 原文 Speak 按鈕永遠 disabled

### Root cause
`src/result/ResultView.tsx` 的 `applyPartialPayload` 跟 `applySnapshot`（loading status 分支）**不分 source**，每次都 `setCacheReady({ original: false, translated: false })`。

Retrans 走 translate_text_streaming 會發 partial events（譯文 streaming 進來），每次 partial 觸發 applyPartialPayload 清 original cacheReady。而 Rust `emit_vlm_event` 在 source == "Retrans" 時**不 prefetch 原文**、**不 emit tts-prefetch-done{original}** → React 的 cacheReady.original 永遠 false → Speak 原文按鈕永久 disabled。

### Fix
`applyPartialPayload` 收的 `VlmPartialEventPayload` 目前只有 source/original/translated。需要用 `source` 欄位判斷。

改 applyPartialPayload：
```
if (p.source === "Retrans") {
    setCacheReady((prev) => ({ ...prev, translated: false }));
} else {
    setCacheReady({ original: false, translated: false });
}
```

applySnapshot 的 loading 分支也一樣判斷 source（snapshot 有 source 欄位）。

applyFinalPayload 已在 T35h 正確處理，不動。

## Bug #2: synthesize_with_voice 極短文字也要很久

### 需要診斷
`src-tauri/src/tts/mod.rs` 的 synthesize_with_voice 每次呼叫新建 tokio runtime + 新 EdgeTtsClient。可能 client init / WebSocket handshake 是瓶頸。

### 加 timing log
synthesize_with_voice 裡用 std::time::Instant 量測三段：
- t0 = Instant::now() 進入時
- 建 runtime 完成 -> log runtime_ms
- EdgeTtsClient::new() 完成 -> log client_ms
- client.synthesize await 完成 -> log synth_ms
- 總 elapsed log

範本：
```
eprintln!("[tts-synth] start voice={} text_len={}", voice_code, text.len());
let t0 = Instant::now();
let runtime = tokio::runtime::Builder::...;
let t1 = Instant::now();
eprintln!("[tts-synth] runtime built elapsed={}ms", t1.duration_since(t0).as_millis());
runtime.block_on(async move {
    let t2 = Instant::now();
    let client = EdgeTtsClient::new()...;
    let t3 = Instant::now();
    eprintln!("[tts-synth] client built elapsed={}ms", t3.duration_since(t2).as_millis());
    let result = client.synthesize(...).await...;
    let t4 = Instant::now();
    eprintln!("[tts-synth] synthesize elapsed={}ms total_from_start={}ms bytes={}",
        t4.duration_since(t3).as_millis(),
        t4.duration_since(t0).as_millis(),
        result.audio.len()
    );
    Ok(result.audio)
})
```

**本 task 只加 log 不改 client 共用** — 等 user 跑一次看 log，決定下一步。

## 非目標
- 不動 Popup UI
- 不動 Retrans 以外的 event flow
- 不動 Settings/Tray
- 不改 EdgeTtsClient 共用（下一顆 T35j 依 log 決定）

## 驗收
- cargo check --all-targets 綠
- npm.cmd run build 綠
- 手動 restart dev：
  * Win+Q 截一段文字 -> 兩個 Speak 按鈕先「合成中…」然後 ready
  * 按 Retranslate -> **Speak 原文保持可按**（不再變合成中）; Speak 譯文短暫合成中再 ready
  * Rust log 顯示 [tts-synth] runtime_ms / client_ms / synth_ms / total_ms 分段時間

## 回報
Phase 1 diff + Phase 2 套改 + build（不碰 git）
CC commit：fix(tts): retrans cacheReady partial path + synth timing log (Stage 7a T35i)
