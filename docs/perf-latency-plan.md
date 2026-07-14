# 截圖後「視窗慢跳出 + OCR 起步慢」— 深度診斷與優化計畫

> 調查方式：4 軌平行深查（熱路徑 / llama runtime / 前端視窗 / 實機 log 量化）。
> 日期基準 2026-07-13。
> **執行進度（2026-07-13 晚）**：階段 1+2 全部完成——T1+T2=`1e9b145`、T4=`bdb294f`（watchdog 實機 kill 測試通過）、T8=`1856528`、T5+T6=`08594fb`、T3=`2bb654c`。每 task 均經 gpt-5.5 adversarial review + 修復。
> **2026-07-14 階段 3 完成（實測驅動）**：T10=`9500cc9`（batch 1024/ubatch 512/fa auto：RAM 17.2→3.9GB、VRAM -2GB、長文 -18%）、T9=`b8b32f2`（GPU vision 預設化，門檻 8GB：長文 7.4s→4.1s、30 輪壓測零洩漏、CPU 尖峰消失）、T7=`ee06f16`（b8955→b9994 + staging install + SSE ping/error 相容）。b9994 上 30/30 壓測綠、watchdog kill 恢復 2 秒。
> 升版研究關鍵翻案：#19639 洩漏是 Gemma 專屬（Qwen3-VL 不受影響）且 fix 已含於 b8955——CPU vision 保險自始非必要。
> 累計效果 vs 7/13 基線：英文長圖 9.2s→4.1s（-55%）、llama RAM 17.2→2.9GB（-83%）、VRAM 7.85→6.6GB。
> 剩餘待辦：T11 prompt cache（可選）、T12 影像 payload（可選、需精度 A/B）、T13-T14 收尾。TTS 缺陷四連修另見 commits 852ba26/9f705bd/740dafb。

---

## 1. TL;DR — 兩個症狀其實是同一個延遲 + 三類事件

**結構性事實**：結果視窗是啟動時預建的隱藏視窗，**只在第一個 streaming token 到達時才被顯示**
（`vlm/mod.rs:533-541` `SHOWN_FOR_SEQ` → `ensure_result_window_visible`；capture / submit 階段沒有任何 code 顯示視窗）。
所以「視窗拖一下才跳出」＝「OCR 等很久才開始」＝**time-to-first-token（TTFT）**，一個數字，兩種體感。

TTFT 被三類事件拉長（全部有實測證據）：

| 類別 | 頻率 | 單次代價 | 根因 |
|---|---|---|---|
| **A. 穩態推論慢** | 每次截圖 | p50 0.8s、**p95 5.7–7.0s**、max 10.8s（server 端實測）| vision encode 在 CPU（`--no-mmproj-offload`，VRAM<16GB 自動觸發）+ `--flash-attn off` + cache 全關 + 影像被 2× nearest 放大（4× vision tokens）|
| **B. server 死掉/重載** | crash ~每 2 天 1 次；另有啟動後首抓、模型切換 | **15–25s**（今日實測 4B 重載）| llama-server b8955 crash（新簽名 `0xc0000409` ucrtbase abort，47 天 23 次）+ **無背景 watchdog**（死掉只在下次熱鍵才被發現）|
| **C. 重載期間零回饋** | 每次 B 發生時 | 使用者以為熱鍵死了 | 「模型啟動中」事件 `emit_to("result")` 但**不顯示視窗**（`vlm/mod.rs:556-562`）→ 動畫在隱藏視窗裡播 |

另一歷史因素：**6/15→7/11 內建 server 幾乎起不來**（port 11434 被 Ollama 佔用，log 中 3/4 spawn 直接 bind 失敗；`525d768` 已改 11500 修掉）——7/11 之前的「怪慢」大多是這條。

---

## 2. 量測證據（不是推測）

- **WER 崩潰記錄**：`AppCrash_llama-server.exe_*` 共 **23 次（2026-05-24→07-09）**；最近 3 次（7/3、7/8、7/9）全是 `0xc0000409`（fail-fast/abort）in `ucrtbase.dll` offset `0xa527e` —— **跟 6 月 RCA 的 `0xc0000005` in llama-common.dll 是不同簽名**。
- **穩態 server 端延遲**（logs/llama-server.log + .log.1，44 筆真實 OCR）：image encode p50=371ms / p90=2404ms / max=7509ms；prefill p50=331ms / p95=2649ms；**total p50≈760ms、p95=5669–7030ms、max=10762ms**。
- **模型重載實測**（今日）：spawn 14:21:40 → 首個 200 回應 14:21:54–14:22:02（期間 4 次 503，client 每 500ms 重試）≈ **15–25s**。
- **watchdog 不存在**：`supervisor.rs` 全檔無 `try_wait` / 背景 health poll；`cc45a4c` 的「watchdog」實際是 **on-demand** `ensure_running()`（只在下次請求失敗時觸發）。`diagnostic.log` 63,289 筆 heartbeat 中 `restart_count` 恆為 0（它只計 500 次閾值重啟，crash 重啟不計）。
- **keepalive**：30 秒一次真實 1-token completion，log.1 中 **4991/5020 筆請求是 ping**（每次 170–340ms GPU prefill；`--parallel 1` 單 slot，撞上時真請求排隊）。
- **VRAM**：快照 9095/12282 MiB（74%，大量第三方 GPU app 共存）；`DEBUG_VRAM_LAG.md` 方案 A（降 batch / 開 flash-attn / cache-ram）**至今未實施**，spawn 參數缺陷 1–4 原封不動。
- **今日 5 次模型切換**（13:12–14:21，2b↔4b），每次 = stop + respawn + 完整重載，熱路徑上同步執行、無 UI 回饋。

### 已排除（查過、不是問題）

- 視窗建立成本：預建隱藏 + close 攔截為 hide，無 per-capture WebView2 冷啟。
- 前端渲染：token 逐字進 textarea、33ms throttle、無 fade-in 動畫；450ms debounce 只管 Speak 按鈕。
- capture 前處理（WGC snapshot / Otsu / bbox / PNG）：~30–150ms，非主因。
- clipboard / captures.log 寫入：在 stream 完成後才發生，不影響 TTFT。

---

## 3. 優化計畫（分階段；每 task = 一個邏輯改動 = 一個 commit，走 codex-collab SOP）

### 階段 1 — 體感即殺：視窗立即出現 + 重載可見（低風險，先做）

| # | Task | 檔案 | 內容 | 驗收 |
|---|---|---|---|---|
| T1 | capture 即顯 + 「辨識中」狀態 | `vlm/mod.rs`、`src/result/ResultView.tsx` | submit 時 emit `vlm-capture-started`（帶 seq）並 `ensure_result_window_visible`（**show 不搶 focus**，首 token 維持現有 focus 行為）；前端新增 `recognizing` 狀態（沿用模型啟動中動畫樣式）；順手讀取 `popup_show_enabled`（目前寫了永遠沒讀）| 熱鍵後 <100ms 視窗可見且顯示辨識中；p95 TTFT 不變但體感消失 |
| T2 | 模型啟動中也要顯示視窗 | `vlm/mod.rs:556-562` | `emit_vlm_model_loading_event` 加 `ensure_result_window_visible`（T1 合併做亦可）| 重載期間動畫可見，不再「熱鍵裝死」 |
| T3 | 模型切換 UI 回饋 | `llama_runtime/mod.rs`、`vlm/mod.rs` | `ensure_model_for_lang` / `switch_model` 觸發時 emit 同一 loading 事件（文案「切換模型中」）| 語言觸發切換時視窗顯示進度而非靜默 15-25s |

### 階段 2 — 消滅「等很久」大宗事件：server 生命週期（低-中風險）

| # | Task | 檔案 | 內容 | 驗收 |
|---|---|---|---|---|
| T4 | 背景 watchdog | `supervisor.rs` | monitor thread 每 ~2s `try_wait()` child；非預期退出（需 expected-stop flag 區分手動 stop/switch）→ 自動 respawn（backoff + 次數上限）+ `RESTART_COUNT` 分開統計 crash 重啟 + 寫 log | 手殺 llama-server 後 ≤5s 自動重生；下次熱鍵不再付 15-25s |
| T5 | restart 後 warmup | `supervisor.rs`、`vlm/mod.rs` | `restart_with_model` / `switch_model` 成功後跑 `warmup()`（現在只有 bootstrap 跑）| 重啟後首抓不付 first-inference 成本 |
| T6 | mid-stream crash 恢復 | `vlm/mod.rs:861-862` | stream read error 屬傳輸層者映射為 `VlmRuntimeDown` → 走既有 `ensure_running` + 單次重試（現在直接報錯收工）| stream 中殺 server，capture 自動恢復而非紅字 |
| T7 | llama.cpp 升版研究（research，先查再動）| `llama_runtime/mod.rs:12` | b8955 在已知 MTMD regression 視窗（#21022/#19639）且新 crash 簽名 0xc0000409 持續發生。查 2026-07 現行 stable tag 對 Qwen3-VL 穩定性、MTMD server fix 與 CUDA vision leak 是否已修 → 拍板升版 | 研究報告 + 目標 tag；升版後 7 天 WER 零新增 |

### 階段 3 — 穩態 TTFT：p95 5.7–7s → 目標 <2s（每項先量測、一次一改）

| # | Task | 檔案 | 內容 | 驗收 |
|---|---|---|---|---|
| T8 | **儀器先行**：TTFT 計時 | `vlm/mod.rs`、`capture/log.rs`、`supervisor.rs` | 記錄 POST→first-delta（TTFT）、window-show 時間戳、per-capture perf line（seq 關聯）；llama-server 加 `--log-timestamps`；`final_duration_ns` 修掉（宣告後從未賦值）| captures.log（或 perf log）每筆有 stage 分解，作為所有後續 before/after 依據 |
| T9 | GPU vision offload A/B | `supervisor.rs:28,91-100` | `C2T_VISION_GPU_OFFLOAD=on` 實測 image encode p50/p95 + 穩定性（VRAM、crash）vs 現狀 CPU；搭配 T7 結果決定預設（12GB 卡的 16GiB 閾值一刀切是目前 p95 主嫌）| A/B 數據報告 → 拍板預設值 |
| T10 | spawn 參數方案 A（DEBUG_VRAM_LAG 欠帳）| `supervisor.rs:412-445` | `--batch-size 1024`、`--ubatch-size 512`、`--flash-attn on`(auto/SM 檢查)、`--cache-ram 256` | VRAM 降 ≥2GB（nvidia-smi 前後對照）；TTFT 不升 |
| T11 | prompt cache | `supervisor.rs` | `--cache-reuse 256`，固定 system prompt（~1.5KB×每次）不再全量重 prefill；驗證對 VLM 請求的實際效果 | TTFT p50 降 ≥200ms 或證偽後 revert |
| T12 | 影像 payload | `vlm/mod.rs:998-1023`、`capture/pipeline.rs` | min-dim <32px 改 **padding** 取代 2× nearest 放大（避免 4× vision tokens）；Q 模式加最長邊 cap（預設 ~1536，可設定）| W/E 細長條 encode 時間下降；OCR 準確率 A/B 不退步 |

### 階段 4 — 次要 + 防回歸

| # | Task | 內容 |
|---|---|---|
| T13 | keepalive 改良 | OCR in-flight / 重啟期間跳過 ping（避免 `--parallel 1` 排隊）；(可選) idle unload 設定項（方案 C）|
| T14 | 防回歸門檻 | perf log 加 p95 監測（diagnostic.log heartbeat 擴充），TTFT p95 > 門檻寫警告；README VRAM/延遲數字更新 |

---

## 4. 需要 user 拍板的決策點

| # | 決策 | 建議 |
|---|---|---|
| D1 | T1 視窗即顯時的 focus 行為 | 建議：show 時**不搶 focus**，首 token 到才 focus（維持現狀的打字不中斷）|
| D2 | GPU vision offload 預設值 | 等 T7+T9 數據再拍板（速度 vs 穩定性取捨）|
| D3 | Q 模式最長邊 cap 數值 | 等 T12 A/B 準確率數據 |
| D4 | idle unload 要不要 | 可選；VRAM 釋放 vs 閒置後首抓 15-25s 的取捨 |
| D5 | llama.cpp 升版目標 tag | 等 T7 研究報告 |

## 5. 執行順序建議

**T1+T2（一個 commit 級別的小改動，體感立即改善）→ T4（crash 大宗）→ T8（儀器）→ T3/T5/T6 → T7 研究 → 階段 3 逐項 A/B → 階段 4。**

每 task 走 codex-collab：CC 寫五欄 spec → Codex 實作 → 雙向 adversarial review → APPROVED 即 commit。階段 3 每項改完用 T8 儀器重新量測、貼 before→after。
