# GPU Resource Leak 修復報告（v0.5.0 → v0.6.0）

> 完整記錄 2026-05-20 至 2026-05-21 期間的 GPU leak 根因分析、修復內容、跨環境設計、與 regression 防護機制。供未來 contributor / maintainer / fork 者參考。

## 1. 症狀

User 回報：Capture2Text 跑久了**整台電腦卡頓**，按 **`SHIFT+CTRL+WIN+B`**（Windows 重啟 GPU display driver 快捷鍵）才恢復。

這個快捷鍵專門用於 driver-level GPU resource 耗盡，意味著問題本質**不是普通 memory leak**，而是 **GPU resource 累積**（VRAM / D3D11 device / DXGI duplication output 等沒被釋放）。

## 2. Root Cause Analysis

派 8 個 Haiku Explore agent 平行研究後，鎖定 3 個 root cause（按信心度）：

| # | 主因 | 信心 | 證據 |
|---|------|------|------|
| 1 | **`xcap 0.9.4` 截圖路徑** | ⭐⭐⭐ 95% | 每次按 `Win+Q/W/E` 都 `Monitor::from_point() + capture_region()` 重建 `IDXGIOutputDuplication` + `D3D11Device`，內部資源沒正確 Release（DXGI 已知 leak pattern） |
| 2 | **llama.cpp MTMD vision CUDA leak** | ⭐⭐⭐ 高 | GitHub issue [#19639](https://github.com/ggml-org/llama.cpp/issues/19639) (CUDA context leak ~700 req 後 OOM)、[#22582](https://github.com/ggml-org/llama.cpp/issues/22582) (vision tower GPU offload bug)、[#19980](https://github.com/ggml-org/llama.cpp/issues/19980) (mmproj 估算缺失) |
| 3 | **WebView2 GDI leak** | ⭐⭐⭐ 中 | WebView2Feedback [#5536](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5536) — GDI 物件 2000+/30 秒累積（達 10000 上限會卡），但 Capture2Text overlay 是 short-lived 影響有限 |

## 3. 修復時間軸（10 commits）

```
e4da04b Phase 1: windows_capture_pool 模組（新檔 322 行）
d7fc0e6 Phase 2: drop-in xcap → windows-capture pool（Cargo.lock 縮 739 行）
2edf13c Phase 3: HTTP client 全域 OnceLock
cd78a6f Phase 5: llama-server 參數 workaround
a76494b Phase 5 refined: env override + DXGI VRAM auto-detect
8882b7d Phase 6: periodic restart + env-configurable
d810786 fix: vlm parse_lenient fallback (順手修 pre-existing TDD gap)
141f28c Task #10: CI release.yml self-managed vcpkg + libjpeg-turbo overlay
afbcfe1 Task #12: ignored stress test (cargo test --ignored gpu_leak_capture_pool)
296dfbf Task #13: self-diagnostic GPU/handle/process log
```

### Phase 1+2 — 根治截圖路徑（主成因）

**問題**：`xcap 0.9.4` 每次熱鍵都 init/destroy DXGI device，內部沒正確 Release。

**解法**：完全拋棄 xcap，改用 `windows-capture` crate 的 **常駐 capture pool** 架構：
- App 啟動時對每個 monitor 啟一個 background capture session（`start_free_threaded`）
- 每個 session 持續收 frame，最新一張存到 `Arc<RwLock<Option<FrameSnapshot>>>` shared buffer
- 熱鍵觸發只讀 shared buffer + BGRA→RGBA swap + crop（零 DXGI 重建）

**Files**: `src-tauri/src/capture/windows_capture_pool.rs` (新 322 行), `screen_capture.rs` (改 `capture_at_cursor`), `lib.rs` (setup 啟 pool, Exit shutdown), `Cargo.toml` (拔 xcap 加 windows-capture)

**經 stress test 實測驗證**（commit `afbcfe1`）：
```
[stress] iter 100→1000: VRAM 穩定 22 MB
[stress] final after shutdown: 0 MB
[stress] growth = 0.00%
```

### Phase 3 — HTTP client 全域共享

**問題**：高頻 3 處每次 `reqwest::Client::new()` / `builder().build()` 累積 TCP TIME_WAIT + socket FD：
- `supervisor.rs:124` keepalive ping（30 秒/次）
- `supervisor.rs:242` `is_healthy()`（vlm hot path 呼叫）
- `vlm/mod.rs:781` `run_streaming_request()`（每次 OCR）

**解法**：在 `supervisor.rs` 加兩個 `OnceLock` helper：
```rust
pub fn shared_async_client() -> &'static reqwest::Client
pub fn shared_blocking_client() -> &'static reqwest::blocking::Client
```
不在 builder 設 `.timeout()`（會吃掉 caller 控制），每個 callsite 用 `.timeout(Duration::from_X(N))` per-call。

低頻路徑（`warmup`/`poll_ready`/`downloader`/`check_for_updates`）保留各自 builder（startup 或極低頻，不需共享）。Azure TTS 已是 struct field 單例。

### Phase 5 + Phase 5 refined — llama-server 跨硬體適應

**Phase 5（commit cd78a6f）**：對 llama.cpp issue #22582 的 workaround
- 加 `--no-mmproj-offload`（強制 vision tower CPU，避 GPU leak）
- 加 `--parallel 1`（單推論隊列，避 slot race）
- 移除 `--no-cache-idle-slots`（允許 KV cache idle 回收）

**問題**：對 24GB+ VRAM user 是純拖慢 200ms（他們本來 GPU offload 不 leak）。

**Phase 5 refined（commit a76494b）**：env override + auto-detect
```rust
fn detect_max_vram_bytes() -> Option<u64> {
    // DXGI IDXGIFactory1::EnumAdapters → DXGI_ADAPTER_DESC.DedicatedVideoMemory
    // 跨 NVIDIA / AMD / Intel
}

fn decide_disable_offload(env: Option<&str>, vram_bytes: Option<u64>) -> bool {
    match env {
        Some("on") => false,   // 強制 GPU
        Some("off") => true,   // 強制 CPU
        _ => vram_bytes.map(|b| b / GB).map_or(true, |gb| gb < 16),
    }
}
```

**Threshold 16 GB**：Qwen3-VL-8B (~8GB) + KV cache (~3GB) + overhead，16GB 才有餘裕 GPU offload。

### Phase 6 — Periodic restart 保險絲

**問題**：即使 Phase 5 強制 CPU offload，llama.cpp #19639 的 CUDA context leak 仍在約 700 req 後 OOM（不同 GPU 觸發點不同）。

**解法**：累積到 threshold 次推論後背景 graceful restart（對 user 透明）。

```rust
static INFERENCE_COUNT: AtomicU64 = AtomicU64::new(0);
static CURRENT_MODEL: OnceLock<Mutex<Option<ModelId>>> = OnceLock::new();
static RESTART_IN_PROGRESS: AtomicBool = AtomicBool::new(false);
static RESTART_COUNT: AtomicU64 = AtomicU64::new(0);

pub fn record_inference_done() { /* increment + check threshold */ }
fn decide_should_restart(count: u64, threshold: u64, in_progress: bool) -> bool {
    threshold > 0 && count >= threshold && !in_progress
}
```

Vlm 在每次推論成功 return 前呼叫 `record_inference_done()`。

### Task #10 — CI release pipeline

**問題**：Windows runner 預裝 vcpkg 是 main HEAD，含 libjpeg-turbo 3.1.4 + MSVC 17.14 觸發 internal compiler error in `jchuff.c`（本機修復時撞過）。

**解法**：CI 內自管 vcpkg + 從 repo `vcpkg-overlay/libjpeg-turbo/` 用 `--overlay-ports` 強制 libjpeg-turbo 3.0.4。

**Files**: `.github/workflows/release.yml` 重寫 + `vcpkg-overlay/libjpeg-turbo/` 新增 5 個 port file。

### Task #12 — In-repo stress test (regression 防護)

`#[ignore]`-gated test 跑 1000 次 `CapturePool::snapshot_at_point` + DXGI3 `QueryVideoMemoryInfo` sample，驗 process VRAM 增量 < 5%。

```bash
cargo test --lib capture::windows_capture_pool::tests::gpu_leak_capture_pool -- --ignored --nocapture
```

預設 CI 不跑（runner 沒 GPU 不適用）；dev 本機 push 前手動跑。

### Task #13 — Self-diagnostic log

App 背景每 60 秒 append JSONL snapshot 到 `app_log_dir/diagnostic.log`：
```json
{"timestamp_epoch":1716284400,"gpu_vram_used_mb":1234,"gpu_vram_budget_mb":12000,
 "gdi_objects":56,"user_objects":12,"working_set_mb":340,
 "inference_count":234,"restart_count":0,"uptime_secs":3600}
```

User 撞 bug 能 export 給 maintainer 快速定位，不需要自己跑 nvidia-smi。

---

## 4. Env Override 完整清單

| Env Var | 預設 | 範圍 | 用途 |
|---------|------|------|------|
| `C2T_VISION_GPU_OFFLOAD` | `auto` | `on` / `off` / `auto` | mmproj GPU/CPU offload 決策。auto 模式根據 DXGI 偵測 VRAM ≥ 16GB 決定 |
| `C2T_LLAMA_RESTART_THRESHOLD` | `500` | 任意 u64 | 累積 N 次推論後 restart llama-server。`0` 完全關閉 |
| `C2T_LLAMA_RESTART_DISABLED` | (未設) | `1` 或未設 | 顯式關閉 periodic restart（更語意明確的開關） |
| `C2T_DIAG_INTERVAL` | `60` | `5` 至 `3600` 秒 | diagnostic log 取樣間隔 |
| `C2T_DIAG_DISABLED` | (未設) | `1` 或未設 | 完全關 diagnostic worker |

## 5. 跨環境支援 Matrix

| 硬體 / OS | Phase 5 預設行為 | Phase 6 預設 | 建議調整 |
|----------|----------------|-------------|----------|
| NVIDIA RTX 30/40 系列 16 GB+ | GPU offload | 500 次 restart | `C2T_LLAMA_RESTART_THRESHOLD=1500` 較少中斷 |
| NVIDIA 8-12 GB（RTX 3060/3070） | CPU offload | 500 次 restart | 預設即可 |
| AMD / Intel ARC（DXGI 支援） | 依 VRAM 決定 | 預設 | 同上，restart threshold 可調 |
| 無 GPU / 純 CPU | DXGI 拿不到 VRAM → 預設 CPU offload | 預設 | `C2T_LLAMA_RESTART_DISABLED=1` 可關（沒 leak）|
| Win 10 1903+ | windows-capture WGC API ✅ | 一致 | 預設即可 |
| Win 11 22H2+ | `IsBorderRequired=false` 去黃框 | 一致 | 預設即可 |

## 6. Regression 防護機制

### A. In-repo stress test
```bash
$env:VCPKG_ROOT='C:\dev\vcpkg'  # dev only
cd src-tauri
cargo test --lib capture::windows_capture_pool::tests::gpu_leak_capture_pool -- --ignored --nocapture
```
未來改 capture path 時必跑，確認 0% growth。

### B. Self-diagnostic log
正常運作時 `%LOCALAPPDATA%\com.capture2text.pro\logs\diagnostic.log` 應該：
- `gpu_vram_used_mb` 在 single OCR session 內穩定波動 ±10%
- `gdi_objects` 不會單調遞增超過 1000
- `working_set_mb` 隨使用緩慢上升（leptonica image buffer cache）但不爆炸
- `inference_count` 達 `C2T_LLAMA_RESTART_THRESHOLD` 時 `restart_count` +1，`gpu_vram_used_mb` 回落

### C. CI release pipeline
`git tag v*` 觸發 `.github/workflows/release.yml` build NSIS installer → 上傳 GitHub Release（draft）。end-user 從 Release page download `.exe` 雙擊安裝。

## 7. 已知 Limit / Future Work

### Phase 7 (defer)：overlay window 用 WS_EX_LAYERED
WebView2 issue #5536 GDI 累積對長時間 hover overlay 影響大，但 Capture2Text overlay 是 short-lived（截圖框 / OCR 結果 mask 500ms 自動 hide），實測影響有限。**不重寫**直到使用者實測撞到 GDI 上限。

未來若要重寫：drag_overlay.rs (37KB) + overlay.rs 從 Win32 `WS_EX_LAYERED` 改成 Tauri 原生 webview window。風險：alpha=0 透明 mouse 模擬點擊邏輯（`PostMessage` to hwnd）需重新設計；p95=2.4ms 性能要保住；多 monitor / DPI scaling 重做。

### Tauri / WebView2 升級風險
WebView2 runtime 自動更新（Edge 帶來的），未來新版可能 fix #5536 或引入新 leak。Self-diagnostic log 的 `gdi_objects` trend 是早期偵測訊號。

### llama.cpp upstream 修復
Phase 5/6 都是 workaround。當 llama.cpp upstream 修了 #19639 / #22582，可以：
- Phase 5: `C2T_VISION_GPU_OFFLOAD=on` 預設（不再強制 CPU）
- Phase 6: `C2T_LLAMA_RESTART_THRESHOLD=0` 預設關閉

**建議定期檢查 llama.cpp release notes**（每 1-2 個月）看是否 upstream 修復。

## 8. Troubleshooting Guide（給 user）

### 症狀 A：跑久了還是卡 / SHIFT+CTRL+WIN+B 仍能修復
1. 開 `%LOCALAPPDATA%\com.capture2text.pro\logs\diagnostic.log`
2. 看 `gpu_vram_used_mb` 趨勢：
   - **單調遞增** → 仍有 leak，回報 issue 附 log
   - **跳到 restart 後回落** → Phase 6 在運作，但 threshold 太大 → 設 `C2T_LLAMA_RESTART_THRESHOLD=300` 降低
3. 看 `gdi_objects` 趨勢：
   - **超 5000 並繼續長** → WebView2 GDI bug 撞到，未來考慮 Phase 7 重寫

### 症狀 B：OCR 慢 200ms（high-VRAM user）
- 你的 VRAM ≥ 16 GB，應該開 GPU offload：`C2T_VISION_GPU_OFFLOAD=on`
- 或檢查 diagnostic log 是否顯示 `[llama-runtime] vision offload = CPU (auto: ... GB VRAM < 16)`

### 症狀 C：app 啟動就 crash
- 看 `app_log_dir/diagnostic.log` 是否有寫（沒寫表示啟動就死，問題可能在 windows-capture pool init）
- 設 `C2T_DIAG_DISABLED=1` 跳過 diagnostic worker 看是否解決
- 設 `C2T_LLAMA_RESTART_DISABLED=1` 跳過 periodic restart 看是否解決

### 症狀 D：朋友裝 installer 起來就死
- 檢查 `%LOCALAPPDATA%\com.capture2text.pro\` 是否被建（沒建 → installer 沒裝對）
- 開 app 等 30 秒看是否進入 first-run download UI（會自動跳 settings tab）
- 模型下載失敗看 settings UI 內 error message

## 9. Maintainer Quick Reference

### 改 capture path 必跑
```bash
$env:VCPKG_ROOT='C:\dev\vcpkg'
cd src-tauri
cargo test --lib capture::windows_capture_pool::tests        # 4 tests (3 + 1 ignored)
cargo test --lib capture::windows_capture_pool::tests::gpu_leak_capture_pool -- --ignored --nocapture
```
看 stress test growth 必須 < 5%。

### 改 llama-server 整合必跑
```bash
cargo test --lib llama_runtime::supervisor::tests   # decide_disable_offload + decide_should_restart
```

### 發新版 release
1. Bump `Cargo.toml` + `tauri.conf.json` + `package.json` 版本（保持一致）
2. Commit version bump
3. `git tag v0.X.Y && git push origin v0.X.Y`
4. CI 自動 build + upload draft release
5. GitHub UI publish release（從 draft → published）

### Build 環境 (dev)
- Windows 10/11 + VS Build Tools 2022 + VCTools workload
- Rust stable (`rustup default stable-x86_64-pc-windows-msvc`)
- Node 18+ + `pnpm` 或 `npm`
- `C:\dev\vcpkg` + 從 vcpkg-overlay 安裝 leptonica:
  ```bash
  cd C:\dev\vcpkg
  .\vcpkg.exe install leptonica:x64-windows-static-md --overlay-ports="<repo>\vcpkg-overlay"
  ```
- `setx VCPKG_ROOT C:\dev\vcpkg`

---

**Last updated**: 2026-05-21 (Capture2Text v0.6.0)
**Related commits**: e4da04b → 296dfbf (10 commits)
**Original incident**: GPU resource leak requiring `SHIFT+CTRL+WIN+B` driver reset
