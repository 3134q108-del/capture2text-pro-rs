# T47 · Partial event 節流 + log 降噪（解決 streaming lag）

## 診斷

Dev log 顯示 VLM streaming 時每個 token 都 emit 三件事：
```
[emit] ensure_result_window_visible: window_exists=true was_visible=true
[emit] vlm-result-partial source=Q original.len=XXX translated.len=XXX
[state] set_partial source=Q
```

從 len=0 到 len=1288（典型中文 + 翻譯），中間會有 **500+ 次 emit**。每次：
- Rust 序列化 partial payload（含整段 text，可達 2-5 KB）
- Tauri IPC 傳到 webview
- React `setPartial` → 整個 ResultView re-render
- `ensure_result_window_visible` 檢查 window / 可能 show window

**結果**：UI thread 被 1000+ 次重複 render 淹沒 → 滑鼠 lag 全機。inference 本身不慢（2.8-11.7s），但 streaming 期間 UI 被卡滿。

## 修正方向

1. **Rust 端節流**：partial emit 間隔最少 120ms，不管 VLM 吐多快
2. **ensure_result_window_visible 早返回**：若已 visible 就不重複做 Tauri API call
3. **log 降噪**：partial / set_partial / ensure_result_window_visible 的 eprintln 降級到「僅在首次或結束時 log」

## 鎖死（MUST）

### 1. `src-tauri/src/vlm/mod.rs`：partial emit 節流

找到 emit `vlm-result-partial` 的函式（`emit_vlm_partial_event` 或類似），在此函式外新增節流邏輯。

策略：用一個 `static PARTIAL_LAST_EMIT: OnceLock<Mutex<Instant>>`，每次 emit 前比較時間：
```rust
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const PARTIAL_THROTTLE_MS: u64 = 120;

static PARTIAL_LAST_EMIT: OnceLock<Mutex<Instant>> = OnceLock::new();

fn partial_throttle_ok(force: bool) -> bool {
    let slot = PARTIAL_LAST_EMIT.get_or_init(|| Mutex::new(Instant::now() - Duration::from_secs(10)));
    let mut last = match slot.lock() { Ok(g) => g, Err(_) => return true };
    if force || last.elapsed() >= Duration::from_millis(PARTIAL_THROTTLE_MS) {
        *last = Instant::now();
        true
    } else {
        false
    }
}

pub fn reset_partial_throttle() {
    if let Some(slot) = PARTIAL_LAST_EMIT.get() {
        if let Ok(mut g) = slot.lock() {
            *g = Instant::now() - Duration::from_secs(10);
        }
    }
}
```

改 emit_vlm_partial_event（實際函式名 Codex 找）：
```rust
fn emit_vlm_partial_event(..., payload: VlmPartialPayload) {
    if !partial_throttle_ok(false) {
        // 丟棄此次 partial，UI 會在 120ms 內等到下次或 final
        return;
    }
    // 原本的 emit 邏輯
}
```

**final event**（`vlm-result` status=success/error）**一定要 emit 不節流**：在該處呼叫 `reset_partial_throttle()` 讓下次新截圖的第一個 partial 立刻通過，不等 120ms。

**新任務開始時**（screenshot submit / retranslate）也 call `reset_partial_throttle()`。

### 2. `src-tauri/src/commands/result_window.rs`：ensure_result_window_visible 早返回 + 降噪

找 `ensure_result_window_visible` 函式：
```rust
pub fn ensure_result_window_visible(...) {
    // 1) 檢查 window visible
    // 2) 若已 visible 就 early return 不 call Tauri API
    // 3) log 改為：只在「從 invisible 變 visible」時 eprintln
}
```

具體改法：
```rust
let window_exists = /* 現有邏輯 */;
let was_visible = /* 現有邏輯 */;

if window_exists && was_visible {
    return;  // 不做 show/focus，不 eprintln
}

eprintln!("[emit] ensure_result_window_visible: exists={} visible={} -> showing",
    window_exists, was_visible);
// 原本的 show 邏輯
```

若不是在 result_window.rs 而在 vlm/mod.rs 內 inline 呼叫，Codex 找實際位置改。

### 3. `src-tauri/src/vlm/mod.rs`：移除 partial eprintln

把每個 partial 的：
```rust
eprintln!("[emit] vlm-result-partial source={} original.len={} translated.len={}",
    ...);
eprintln!("[state] set_partial source={}", source);
```
**刪掉**或**降級**：
- 把這兩條 eprintln **全部刪掉**（streaming 期間不印）
- 只保留 **final success** 一次 log：`[vlm] source={} duration_ms: {}`（這個現有的留著，已經是 final 了）
- 保留 **error** 路徑的 eprintln

### 4. React 端（可選但推薦）：ResultView partial update 用 requestAnimationFrame 節流

若 T47 §1 已在 Rust 節流到 120ms，React 端影響較小，可**跳過本項**。
若 Rust 節流效果不夠，再考慮 React 端用 `useDeferredValue` 或手寫 throttle。

## 禁動

- **不動** VLM inference 邏輯 / keep_alive / warmup / Ollama boot
- **不動** final event (vlm-result success/error) 的 emit — 一定要準時
- **不動** clipboard / window-state-changed / output-language-changed
- **不動** TTS / hotkey / capture

## 驗證

- `cargo check` + `cargo build` 通過
- `npm build` 通過
- UTF-8 NoBOM

## 回報

```
=== T47 套改結果 ===
- vlm/mod.rs partial emit 節流 120ms + reset 點
- vlm/mod.rs 移除 partial / set_partial eprintln
- ensure_result_window_visible 早返回 + 降噪
- cargo check: <結果>
- UTF-8 NoBOM

VERDICT: APPROVED
```

**直接套改**。UTF-8 NoBOM。
