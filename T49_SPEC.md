# T49 · VLM 連按機制改「丟舊保新」（方案 A）

## 目標

目前 VLM 佇列 `sync_channel(4)` 是 FIFO。連按 Win+Q 時舊截圖會排隊依序跑，user 只想看最新那張翻譯卻得等。

改成：
- **處理中**的 job 繼續跑（不中斷 inference）
- **Pending slot 只保留 1 個最新 job**（新 submit 進來會覆蓋舊 pending）
- worker 處理完當前 job → 看 pending slot → 若有就抓起來跑 → 跑完再看 → 沒就睡

## 鎖死（MUST）

### `src-tauri/src/vlm/mod.rs`：改 worker 架構

**砍掉** 現有：
- `use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};`
- `VLM_QUEUE_CAPACITY = 4`
- `sync_channel::<VlmJob>(VLM_QUEUE_CAPACITY)`
- `try_send` 邏輯

**改用** 單 slot + Condvar：

```rust
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread::{self, JoinHandle};

struct WorkerShared {
    pending: Mutex<Option<VlmJob>>,
    cv: Condvar,
    stop: Mutex<bool>,
}

struct WorkerHandle {
    shared: Arc<WorkerShared>,
    join: Option<JoinHandle<()>>,
}

static WORKER: OnceLock<Mutex<Option<WorkerHandle>>> = OnceLock::new();
```

**submit 邏輯**：
```rust
fn try_submit(job: VlmJob) {
    let slot = WORKER.get_or_init(|| Mutex::new(None));
    let guard = slot.lock().expect("WORKER lock poisoned");
    let Some(worker) = guard.as_ref() else {
        eprintln!("[vlm] worker not initialized, dropping request");
        return;
    };

    let mut pending = match worker.shared.pending.lock() {
        Ok(g) => g,
        Err(_) => { eprintln!("[vlm] pending lock poisoned"); return; }
    };
    if pending.is_some() {
        eprintln!("[vlm] replacing pending job (drop-old keep-new)");
    }
    *pending = Some(job);
    drop(pending);
    worker.shared.cv.notify_one();
}
```

**worker thread loop**：
```rust
fn worker_loop(shared: Arc<WorkerShared>) {
    loop {
        // 等 pending 或 stop
        let job = {
            let mut pending = shared.pending.lock().unwrap();
            while pending.is_none() {
                if *shared.stop.lock().unwrap() { return; }
                pending = shared.cv.wait(pending).unwrap();
            }
            pending.take()
        };
        if let Some(j) = job {
            handle_job(j); // 原本的 process 邏輯
        }
    }
}
```

**init / shutdown**：
- `init_worker()`：建 `Arc<WorkerShared>`，spawn thread，寫進 WORKER
- `shutdown_worker()`（若原本有）：設 stop=true，notify_one，join thread

### 保留不動

- `handle_job(job)`：原本 worker 從 channel recv 後的處理邏輯（含 inference / emit partial / emit final），**完全不動**，只改 channel → slot
- 所有 emit / partial 節流 / clipboard / log 邏輯
- `VlmJob` enum / `VlmOutput` / `VlmPayload` 等型別
- `try_submit_ocr` / `try_submit_text` 介面（只是 wrap `try_submit`）

## 禁動

- **不中斷** in-flight inference（方案 A 特性：現跑的一定跑完）
- **不改** reqwest client / streaming 邏輯
- **不影響** TTS / clipboard / tray / cross-window-sync

## 驗證

- `cargo check` + `cargo build` 通過
- `npm build` 通過（雖然只改 Rust，仍驗無 regression）
- UTF-8 NoBOM

## 風險

1. **Condvar + spurious wake**：loop 內的 `while pending.is_none()` 已處理 spurious wake
2. **stop 路徑**：需確認 shutdown 呼叫點（現有 `[shutdown] begin` log），若原本 drop channel tx 作為 stop 訊號，改成 `*shared.stop.lock() = true; cv.notify_one();`
3. **in-flight job 無法取消**：這是方案 A 預期行為（user 看完跑完的舊結果，再被新結果取代）。若 user 未來要「秒中斷舊 job」是方案 B，不在本輪

## 回報

```
=== T49 套改結果 ===
- vlm/mod.rs: channel(4) -> WorkerShared { Mutex<Option<VlmJob>> + Condvar }
- try_submit 改覆蓋式寫入
- worker_loop 改 condvar wait
- cargo check / cargo build: <結果>
- UTF-8 NoBOM

VERDICT: APPROVED
```

**直接套改**。UTF-8 NoBOM。
