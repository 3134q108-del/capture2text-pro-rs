# T46 · keep_alive 從 30m 降成 5m 避免全機 lag

## 問題

User 回報：T44/T45 後框選操作讓整台電腦超級 lag（以前同流程不會）。

## 診斷

T44 加 `const KEEP_ALIVE: &str = "30m"` 讓 qwen3-vl:8b-instruct（7.4 GB）**持續**佔 GPU 30 分鐘。

Ollama **預設是 5 分鐘閒置後 unload**，所以 user「以前」（沒設 keep_alive 或設預設）體感沒 lag — 模型只在使用時佔 GPU，用完釋放。

30 分鐘常駐 = GPU 大部分時間被 Chromium compositor / 其他應用跟 Ollama context 搶，導致 UI 卡頓。

## 修正

**`KEEP_ALIVE: "30m" → "5m"`**（貼近 Ollama 預設）。這樣：
- 使用中：模型常駐，連續截圖秒回
- 5 分鐘沒用：unload 釋放 GPU，其他應用恢復流暢
- 下次用時觸發一次 cold start（但 warmup 和 `REQUEST_TIMEOUT_MS=90_000` 還在 fallback）

保留 `warmup()` 在 setup 不動（啟動時提前載入，第一次 Win+Q 秒回）。

## 鎖死（MUST）

### `src-tauri/src/vlm/mod.rs`

```rust
const KEEP_ALIVE: &str = "5m";   // 原本 "30m"；30m 會讓 GPU 持續被佔，全機 lag
```

只改這一行。其他不動。

## 禁動

- 不動 `REQUEST_TIMEOUT_MS`（保留 90s 當 fallback）
- 不動 warmup
- 不動 T45 ollama_boot 邏輯
- 不動其他模組

## 驗證

- `cargo check`
- **不需**跑 cargo build（只改常數）
- **不需**跑 npm build

## 回報

```
=== T46 套改結果 ===
- vlm/mod.rs: KEEP_ALIVE "30m" -> "5m"
- cargo check: <結果>
- UTF-8 NoBOM

VERDICT: APPROVED
```

**直接套改**（一行常數改動）。UTF-8 NoBOM。
