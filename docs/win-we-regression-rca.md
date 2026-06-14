# Capture2Text Win+W/E 框選 + 模型故障 — 根因調查與修復計畫

> 調查方式：5 軌平行深查（座標/DPI、WGC pool、git 時間軸、VLM 模型、overlay）→ 各軌對抗式驗證 → 綜合。
> 本報告已整合 user 後續補充的三條關鍵線索（見下）。日期基準 2026-06-14。
> **狀態：診斷 + 修復計畫。尚未改任何 code。**

---

## 0. User 補充線索（workflow 啟動後才得知，已併入結論）

| 線索 | 對結論的影響 |
|---|---|
| **VLM 壞掉是「紅字報錯」**（不是亂碼/假成功）| 推翻「寬鬆 parser 偽成功（根因 C）是你的模型症狀」——C 是把錯誤**藏起來**，而你看到的是**明確錯誤**。C 仍是該修的技術債，但**不是你這次的模型故障主因**。 |
| **要「切換模型 / app 重啟」才能修好，之後又會壞** | 典型**累積型 server 卡死 / 洩漏**特徵。`switch_model` 與 app 重啟都 = 砍掉重生 llama-server（`llama_runtime/mod.rs:36`、`supervisor.rs:260`），所以恢復點在 **server 端**。 |
| **Win+Q 完全正常** | Q 走**舊 GDI BitBlt** 路徑（`capture_rect_via_gdi`），完全繞開 WGC pool + bbox 自動偵測；且 Q 與 W/E **送同一台 llama-server**。Q 正常證明：(1) overlay 與一般座標→螢幕路徑沒問題（框選 bug 鎖死在 WGC pool 專屬路徑）；(2) llama-server 本體沒壞，差別在 **W/E 餵的圖**（寬細長條 vs Q 的緊湊矩形）。 |

**兩個獨立故障、兩個獨立恢復路徑**（互相印證）：
- **框選錯** → 只有 app 重啟能修（重啟 = 重建 WGC pool → 拿到新影格）；切換模型**不會**修框選（它不碰 capture pool）。→ 指向 capture pool 的 **stale frame**。
- **模型紅字** → 切換模型 **或** app 重啟都能修（兩者都重啟 llama-server）。→ 指向 **server 卡死**。

---

## 0.5 更新（2026-06-14）：紅字已實機確認 = llama-server 掛掉、且不會自動重生

實機重現，紅字為 **`llama-server connection refused (is runtime running?)`** = `VlmError::VlmRuntimeDown`（`vlm/mod.rs:36-37`）。此錯誤只在 `map_reqwest_send_error`（`vlm/mod.rs:961-969`）判定 `err.is_connect()` 時回傳 → **TCP 連 127.0.0.1:11434 被拒 = llama-server 沒在聽 = server process 掛了**。

**這修正了下方原根因 B（CPU vision 慢→timeout）**：慢的話錯誤會是 `timed out after 90000ms`（`Timeout`），不是 connection refused。所以模型症狀**不是「慢」、是「server 死掉」**。

### 新的模型主因（高信心）：server 掛掉 + supervisor 無自動重生

`supervisor.rs` 經 grep 確認**沒有任何 watchdog / 死亡偵測 / 自動重生**：
- `spawn_with_paths`（`:318-365`）用 `CREATE_NO_WINDOW` 起 child，**沒有 `.stderr()/.stdout()` 導向** → llama-server 自己的 crash log **完全沒被收**（死因無從得知 = 觀測性缺口）。
- 全檔只有 `stop_current_server` 的 `child.wait()`（`:440`），**沒有任何 thread 對 child 做 `try_wait()`** 偵測非預期退出。
- `start_keepalive`（`:287-316`）每 30s ping 一次但**忽略結果、不重生**（只保溫，不是 watchdog）。
- 唯一重生路徑：① app 啟動 ② **使用者手動切換模型** ③ 第 500 次「**成功**」推論的週期重啟（`record_inference_done` `:140`）。

→ server 一旦死掉就**卡在 connection refused 直到你手動切模型 / 重啟 app**，完全對上你的症狀（含「Q 正常」：Q 是在 server 還活著時測的；server 死後 Q 也會同樣 refused）。

### server 為何死掉（兩候選，下方測試二選一）

| 候選 | 機制 | 歸屬 |
|---|---|---|
| **H1 crash/OOM** | #19639 CUDA MTMD leak 或 b8955 mtmd bug → VRAM OOM → llama-server crash（在到 500 次成功之前就死）| 死因 pre-existing，被 campaign 的 arg 改動可能放大 |
| **H2 週期重啟失敗** | `8882b7d` 第 500 次成功推論觸發重啟 → `stop_current_server`（殺 child）→ `restart_with_model` 若失敗只 log 不重試（`:195`）→ server 永久 down | `8882b7d`（Phase 6，新加）|

### 死因已實機確認（2026-06-14）：**H1 鎖定 — llama-server 原生 crash（0xc0000005）**

實機在紅字當下用 PowerShell 觀測 + Windows 事件記錄檔，**確認 H1、排除 H2**：

- **紅字當下**：`capture2text-pro-rs`（app, PID 15748）還活著，但 **`llama-server.exe` 不在**、**port 11434 無人 listen** → 確認「connection refused = server 已死且 app 沒重生」。
- **Windows Application Error（Event ID 1000 / WER 1001）**，今日 **10:23 與 11:12 各一次以上**：
  ```
  失敗的應用程式: llama-server.exe
  錯誤模組:       llama-common.dll
  例外狀況代碼:   0xc0000005    ← ACCESS VIOLATION（原生 segfault）
  錯誤位移:       0x000000000014a22d
  路徑: C:\Users\Home\AppData\Local\com.capture2text.pro\bin\llama-server.exe
  crash dump: C:\ProgramData\Microsoft\Windows\WER\ReportArchive\AppCrash_llama-server.exe_...
  ```
- **判讀**：`0xc0000005` 是**硬性原生記憶體存取違規**，發生在 **`llama-common.dll`**（llama.cpp `b8955` build 內部）。這是 **llama.cpp 本身的 crash**，不是被 Phase 6 乾淨 kill（那是 TerminateProcess、不會產生 APPCRASH），也不是被優雅處理的 OOM。`--no-mmproj-offload`（把 vision 丟 CPU）**沒能擋住**它（fault 在 `llama-common.dll`、非 CUDA vision graph）。一個 session（08:08 起）內 crash ≥2 次，與「W/E 用一陣子就掛、要重啟、又掛」完全吻合。
- 對應 `LLAMA_CPP_TAG = "b8955"`（`llama_runtime/mod.rs:12`）落在知識庫標記的 b8807–b8955 server-side MTMD regression 視窗（#21022 / #19639 家族）。

### 模型側雙層缺陷（最終）

| 層 | 缺陷 | 控制權 | 修法 |
|---|---|---|---|
| **上游（為何死）** | llama.cpp `b8955` 在 W/E 多模態請求路徑 segfault（`llama-common.dll` 0xc0000005）| 不在我們（但可換 build / 改參數）| 換到已修 MTMD 的 llama.cpp build（**需研究驗證**哪個 tag 對 Qwen3-VL 穩定）；或改啟動參數避開 crash path |
| **app（為何卡死不恢復）** | supervisor 無 watchdog → 一次 crash = 永久 down 直到手動重啟 | **完全在我們** | **加 watchdog 自動重生 + 收 llama-server stderr** |

> **app 側 watchdog 是最高槓桿、且完全在我們掌控**：就算上游 crash 短期內無法消滅，watchdog 一上，「要手動切模型/重啟」這個症狀**直接消失**（crash 後幾秒內自動重生）。這應是第一個動的 code。原 V0–V4 測試已被本次實機觀測取代、無需再跑。

### 模型側修法（取代下方原階段 4 的偏重）

- **必做（修症狀根源）**：`supervisor.rs` 加 watchdog —— monitor thread 對 child `try_wait()`，或 health-check 失敗 → 自動 `restart_with_model`（帶 backoff + 次數上限）。讓「server 死掉要手動重啟」**直接消失**，不論死因 H1/H2。
- **必做（修觀測性）**：`spawn_with_paths` 把 child stderr 導到 rotating log 檔 → 下次直接看得到死因（OOM vs assert vs mtmd）。
- **若 H2 鎖定**：`spawn_restart_task` 的 `restart_with_model` 失敗要重試 + 期間 submit 排隊（併入階段 2 的 restart-aware guard）。
- **若 H1 鎖定（OOM）**：回到「GPU 洩漏穩定性 vs 速度」取捨（原階段 4），可縮短重啟閾值或改 VRAM 水位觸發；watchdog 仍是必須的安全網。

---

## 1. 摘要 (TL;DR)

「Win+W/E 框選不正常 + 模型故障」是**兩條獨立缺陷鏈，被同一批 commit（v0.5.0→v0.6.0 GPU 洩漏修復，2026-05-20/21）同時引入**，所以表現上一起壞。這完全對上你說的「上次動過 W/E 邏輯 + 同時調了模型，那次兩個都出事」。

**最重要的一個否定結論**：原本領頭的「125% DPI 座標縮放錯誤」假說**被推翻**。本 app 由 tao 0.34.8 在啟動時設成 **Per-Monitor-V2 DPI 感知**，此模式下游標、monitor 原點、WGC 影格三者全是同一套 physical pixel——**單螢幕 125% 不會有 1.25 倍偏差**。**任何「乘/除 1.25」的修法都是錯的，會引入真 bug。**

排序後的真根因：

| # | 根因 | 屬於 | 信心 | 對應 commit |
|---|---|---|---|---|
| **A** | **Stale WGC frame**：pool 讀「最新快取影格」，但 `captured_at` 新鮮度被讀進來後直接丟棄；靜態畫面下 WGC 不送新影格 → 框選/OCR 跑在過時像素 | 框選（主因）| 中（需現場確認頻率）| `d7fc0e6` |
| **B** | **【已被紅字推翻→改寫，見 §0.5】** 紅字實為 `connection refused` 非 timeout → 模型主因改為 **llama-server 掛掉 + supervisor 無自動重生**（H1 crash/OOM 或 H2 週期重啟失敗）；CPU vision 慢（`--no-mmproj-offload`）至多是次要拖慢 | 模型（主因）| **高**（紅字實證）| `8882b7d`（H2）/ 死因待 §0.5 測試 |
| **C** | **寬鬆 parser 偽成功**：回退了先前刻意移除的 lenient fallback，模型吐非 JSON 時靜默當成功 | 模型可觀測性（**你應該不是中這條**，因為你看到紅字）| 高（機制存在）但對本案次要 | `d810786` |
| **D** | **週期重啟競態**：每 500 次推論重啟 llama-server，submit 端無 health guard → 長 session 偶發 connection refused | 穩定性（偶發、非主因）| 中 | `8882b7d` |

`d7fc0e6` 是唯一改動 W/E capture 邏輯的 commit（之後 `pipeline.rs`/`screen_capture.rs` 座標數學凍結）。

**第一步不是改 code，是收集證據**：① 抄下紅字確切文字（決定 B/D/餵壞圖 哪條）② 跑一次帶 log 的 Win+W（驗 DPI 推翻 + stale frame）③ `C2T_VISION_GPU_OFFLOAD=on` 對照（驗 B）。

---

## 2. 故障現象 vs 根因鏈 (RCA)

### 根因 A — Stale WGC frame（框選主因）｜信心：中

- **Failure**：藍框框在「文字曾在、現已不在」的位置 / 大小怪。
- **Infection**：游標移到文字上（移動時 cursor 合成會觸發影格）→ 手移開、畫面靜止 → WGC 在靜態畫面停止送新影格 → 按 Win+W → `snapshot_at_point()` 回傳**最後一張可能過時的快取影格**（`windows_capture_pool.rs:220-231`）→ `capture_at_cursor` 用「當下游標座標」在「過時影格」上裁切（`screen_capture.rs:78-102`）→ bbox 偵測/回代/overlay/送 VLM 全跑在過時像素。
- **Defect**：`d7fc0e6` 把 xcap「每次按鍵同步新抓」改成 WGC pool「讀最新快取」，且 `screen_capture.rs:76` 把 `let _captured_at = snapshot.captured_at;` 讀出後**直接丟棄**，無 max-age guard；`screen_capture.rs:55-70` 的 5×30ms retry **只在「從沒收過影格」(`Ok(None)`) 時重試，對「有影格但過時」無能為力**。
- **與線索一致**：只有 **app 重啟**（重建 pool）能修框選、切換模型不能修——正是 stale-pool 的特徵。
- **Counterfactual**：`C2T_DEBUG_SAVE=1` → 靜態文字 → idle 2–3 秒 → 不動滑鼠、用鍵盤換掉內容 → 立刻 Win+W → 看存下的 PNG（`screen_capture.rs:125-133`）。若是舊內容 → 證實。加入「`captured_at.elapsed() > ~100ms` 視為 `Ok(None)` 重試」後消失。
- **保留不確定**：移動中的互動會自我緩解（游標合成持續送影格），故可能是**間歇**而非每次必現。需現場量測頻率。

### 根因 B — Vision tower 被強制丟到 CPU（模型主因候選）｜信心：中

- **Failure**：OCR 紅字報錯（很可能是 `timed out after 90000ms` 或 server 卡住），要重啟才繼續。
- **Infection**：`should_disable_gpu_offload()`（`supervisor.rs:83-118`）預設 `auto` → `decide_disable_offload`（`supervisor.rs:72-81`）在 VRAM < 16 GiB（**偵測失敗 `None` 也算**）回 `true` → 啟動加 `--no-mmproj-offload` → vision encoder 跑 CPU。**W/E 的寬細長條（W=775px、E=1500px，且 <32px 高會被 nearest 放大成怪比例）→ vision patch 數多 → CPU encode 慢 → 撞 90s `REQUEST_TIMEOUT_MS`**；`--parallel 1` 下單 slot 被慢請求塞住，後續 W/E 也跟著卡 → 重啟才清。**Q 的緊湊矩形 patch 少 → 來得及完成 → 正常**（解釋 Q-vs-W/E 差異）。
- **Defect**：`cd78a6f`（無條件加）+ `a76494b`（改條件式）把 vision tower 推上 CPU。commit message 引用的 llama.cpp **#22582 是誤植**——其真實標題是「MTMD Vision extremely slow: ~82 seconds per image slice」，是抱怨 CPU vision 慢的效能 bug，`--no-mmproj-offload` 正好強制進入它抱怨的慢路徑。來源：https://github.com/ggml-org/llama.cpp/issues/22582
- **取捨平衡**：同 commit 也引用 **#19639（CUDA MTMD requests loop 記憶體洩漏，~700 requests OOM）**——這才是整個 campaign 真正動機（對應 `DEBUG_VRAM_LAG.md`）。把 vision 留 CPU **確實緩解 CUDA vision 洩漏**，所以這是「GPU 洩漏穩定性 vs OCR 速度」的取捨，被誤植的 #22582 掩蓋。`--no-mmproj-offload` 是**正確性保留**的（只搬 projector 到 CPU、輸出不變、只是慢）→ 造成「慢/卡」非「亂碼」，與你「紅字報錯」相符。來源：https://github.com/ggml-org/llama.cpp/blob/master/docs/multimodal.md
- **Counterfactual**：`C2T_VISION_GPU_OFFLOAD=on`（跳過 `--no-mmproj-offload`）重跑 W/E，比較 `duration_ms` 與是否還紅字；切回 `auto`/`off` 必須重現慢/卡。看 stderr `[llama-runtime] vision offload = ...` 確認走哪條。

### 根因 C — 寬鬆 parser 偽成功（可觀測性債，**你應該不是中這條**）｜信心：高（機制）/ 本案次要

- **機制**：模型吐非 JSON 時 `parse_model_output`（`vlm/mod.rs:919-926`）回 `Ok(原始字串塞 translated)` 而非 `Err`，被當成功寫進 log/clipboard（`vlm/mod.rs:477-480`）。`d810786` 重新引入了 `be0433d`（5/13）刻意移除的此 footgun。
- **為何判定非你的症狀**：C 的表現是**假成功**（顯示空 original + 怪 translated），不是**紅字錯誤**。你明確說是紅字 → 你走的是真 `Err` 路徑（B/D/餵壞圖），不是 C。
- **仍建議修**：C 會在「串流被取消留半截 JSON」或「空白 crop 吐空 JSON」時把真故障藏起來，**害你更難診斷 A/B**。屬低風險清債。

### 根因 D — 週期重啟競態（偶發、非主因）｜信心：中

- **機制**：`record_inference_done()`（`supervisor.rs:140`）第 500 次成功推論觸發背景重啟（`spawn_restart_task` `supervisor.rs:172-203`）→ `stop` + 重生 + `poll_ready`（最久 300s）。submit 路徑（`vlm/mod.rs:718-795`）**沒有 `is_healthy()`/`RESTART_IN_PROGRESS` 檢查**就 POST → 重啟視窗內按 W/E → 打到死 socket → connection refused。
- **為何非主因**：需 500 次「成功」推論才觸發；若你的 W/E 一直紅字（少有成功），多半到不了 500。對「一改完就壞、且要靠重啟」的描述吻合度低，列偶發次因。
- **Counterfactual**：`C2T_LLAMA_RESTART_THRESHOLD=0` 關閉週期重啟，看偶發 refused 是否消失；或設 3 猛按 W/E，第 3 次邊界必現。

### 已排除（驗證後確認非根因）

- **純 DPI 1.25 縮放錯誤——推翻**：tao 0.34.8 在 `EventLoop::new()` → `become_dpi_aware()` → `SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2)`，發生在 `lib.rs:194 .build()`，**早於** `lib.rs:104 .setup()` 啟動 capture pool。PMv2 下 `GetPhysicalCursorPos`、`GetMonitorInfoW.rcMonitor`、WGC 影格全 physical，單螢幕 125% 誤差歸零。**不要套任何 ×1.25。**
- **overlay 定位**：`overlay.rs` 自 `474b1f4`（regression 前一個月）byte-for-byte 未改；Win+Q 用**完全相同**的 `std::thread` + `SetWindowPos` + `GetPhysicalCursorPos`，Q 正常 → 若 overlay 有 ×1.25 錯，Q 會同樣壞。
- **`fa3834c`（Task #17）**：`git show` 全是 logging/UI，無任何 crop/monitor 座標數學。`git log d7fc0e6..HEAD -- pipeline.rs screen_capture.rs` 為空。
- **BGRA→RGBA stride/shearing**：`as_nopadding_buffer` 逐行剝 padding，`RgbaImage::from_raw` 用 content w/h，無剪切。
- **llama.cpp 二進位版本**：`LLAMA_CPP_TAG b8870→b8955` 在 `63f7d10`（4/28，視窗外）改的，不在這批。

---

## 3. Regression 時間軸

整批 = v0.5.0→v0.6.0 GPU 洩漏修復，2026-05-20/21，連續 7 commit。`git log e4da04b~1..HEAD` 確認是該視窗內唯一的 capture/llama 改動。

| Commit | 日期 | 軌道 | 改了什麼 | regression? |
|---|---|---|---|---|
| `e4da04b` | 05-20 | 框選 | Phase 1：WGC pool scaffolding | 兩來源座標結構誕生（未上線）|
| **`d7fc0e6`** | 05-20 | **框選** | Phase 2：xcap → 常駐 WGC pool；`capture_at_cursor` 重寫 | **✅ 框選軌道唯一 regression** |
| `2edf13c` | 05-20 | — | Phase 3：共用 reqwest client | 無關 |
| `cd78a6f` | 05-20 | 模型 | Phase 5：無條件加 `--no-mmproj-offload` 等 | 引入 CPU vision 路徑 |
| **`a76494b`** | 05-20 | **模型** | Phase 5 refined：改條件式（VRAM<16GB→CPU offload）| **✅ <16GB 卡進 CPU vision** |
| `8882b7d` | 05-21 | 模型/穩定 | Phase 6：每 500 次推論週期重啟 | ✅ submit 無 guard → 偶發競態 |
| `d810786` | 05-21 | 模型/可觀測 | 寬鬆 parser fallback（回退 `be0433d`）| ✅ 偽成功遮蔽（本案次要）|
| `fa3834c` | 後 | — | Task #17 split capture logging | ❌ 純 logging，已排除 |

**`d7fc0e6` BEFORE→AFTER**：BEFORE `monitor.x()/y()/width()/height()` 四值全來自同一個 xcap `Monitor`、每次按鍵同步新抓；AFTER `monitor_x/y` 來自 `GetMonitorInfoW.rcMonitor`、`monitor_w/h` 來自 WGC 影格、且 `captured_at` 被丟棄。

---

## 4. 需要現場確認的事項（先驗證，再改 code）

> 全部用環境變數 / 既有 debug 開關 / 一行 log，零或低風險。

| # | 動作 | 看到什麼 = 證實什麼 | 優先 |
|---|---|---|---|
| **V0** | **下次重現時抄下紅字整行** | `timed out after 90000ms` → **B**（CPU vision 太慢塞 slot）；`connection refused` → **D** 或 server OOM；`HTTP 500` → server 端崩；`image preprocessing failed`/`response decode failed` → 餵了壞圖（**A** 的下游）| ★★★ |
| **V1** | 在 `screen_capture.rs:72-79` 暫加一行 log：`monitor_x/y/w/h, cursor.x/y, captured_at.elapsed()`，跑一次 Win+W | `monitor=(0,0,1920,1080)` + cursor 在範圍內 → DPI 推翻成立；`elapsed()>100ms`（靜止時）→ **A 成立** | ★★★ |
| **V2** | `C2T_DEBUG_SAVE=1` + 第 2 節 A 的 counterfactual 操作，看存下的 PNG | PNG 是舊內容 → 證實 **A** 並估頻率 | ★★ |
| **V3** | `C2T_VISION_GPU_OFFLOAD=on` 重跑 W/E，對照 `auto` 的 `duration_ms` + 是否還紅字 | `on` 變快/不紅字 → 證實 **B** | ★★★ |
| **V4** | `C2T_LLAMA_RESTART_THRESHOLD=0` 跑長 session | 偶發 refused 消失 → 證實 **D** | ★ |
| **V5** | 把「V2 存下但內容正確」的 crop 直接餵 `ocr_and_translate` | 正確 crop → 正確 OCR，則證明引擎沒壞、模型症狀來自 A 餵壞圖 + B 慢 | ★★ |
| **V6** | 確認是否有**第二螢幕**且不同縮放 | 有 → 階段 5 硬化才有現場價值；無 → 多螢幕發散假說不適用 | ★ |

---

## 5. 修復計畫（分階段，外科手術式）

每階段 = 一個邏輯改動 = 一個 commit。**先低風險高確定，需先驗證的排後面。禁套任何 ×1.25 DPI 縮放。**

### 階段 0（零 code）— 跑 V0–V5
先用第 4 節確認 A/B/D 的實際發生與頻率。**這是第一步。**

### 階段 1（最低風險）— 還原嚴格 parser（修 C 債、恢復錯誤訊號）
- **檔**：`vlm/mod.rs`（`parse_model_output` ~`:919-926`；移除 `parse_lenient_*` 測試）。
- **改**：無 JSON object → 回 `Err(ResponseDecode)`（還原 `be0433d`）；空 `original`/空 `translated` 的退化結果不寫 clipboard/log。
- **為什麼**：先讓真錯誤現形，A/B 才看得見。**先做這個**，後續 V0 的錯誤文字才完整可信。
- **風險**：低（改回先前驗證過的行為）。

### 階段 2（低風險）— submit 加 restart-aware guard（修 D）
- **檔**：`supervisor.rs`（曝露 `restart_in_progress()`/`is_healthy()`）+ `vlm/mod.rs`（`run_streaming_request` POST 前）。
- **改**：POST 前若重啟中 → 等 `poll_ready`/`is_healthy` 再送（短暫排隊），不打死 socket。設等待上限避免 hotkey 永久卡。
- **風險**：低。

### 階段 3（中風險、需 V2 證實 A）— stale frame 新鮮度 guard（修框選主因）
- **檔**：`screen_capture.rs`（`capture_at_cursor`，停止丟棄 `captured_at`）。
- **改**：`snapshot.captured_at.elapsed() > 閾值(~100-150ms)` → 視為 `Ok(None)` 走既有 retry 等新影格；retry 後仍過時 → fallback 一次性同步抓（既有 `capture_rect_via_gdi` 走 crop region，永遠最新）。**不再靜默回任意舊影格。**
- **風險**：中。閾值依 V2 量測調。GDI fallback 在 PMv2 下 physical、安全。

### 階段 4（中風險、需 V3 + 你決策）— 重評 vision offload 取捨（修模型主因）
- **檔**：`supervisor.rs`（`decide_disable_offload` + 修正註解誤植的 issue 編號）。
- **改**：依 #19639（真洩漏）vs #22582（被誤植的效能 bug）重新校準。最小可逆步：先確認 `C2T_VISION_GPU_OFFLOAD=on` 對你的卡可用且穩定（V3），再決定是否改預設/閾值。**也可考慮：保留 GPU vision + 靠 Phase 6 週期重啟壓洩漏**（速度優先），或縮短重啟閾值。
- **風險**：中。**屬「GPU 洩漏穩定性 vs OCR 速度」業務取捨 → 需你拍板，我不單方改預設。**

### 階段 5（低風險、防禦性、需 V6）— pool 兩來源座標硬化
- **檔**：`windows_capture_pool.rs`（`monitor_geometry` 在 `:266-267` 已算出 rcMonitor 寬高卻被 `snapshot()` 丟棄改用 WGC 影格）。
- **改**：讓 `contains_point`/bounds 的尺寸與原點同源；或 session 啟動 assert `rcMonitor size == WGC frame size`，不一致就 log/skip（抓多螢幕混合 DPI / 執行期改解析度 / 熱插拔幾何過時）。順手把 startup prime 改成 prime 所有 monitor（`lib.rs:106` 目前只 prime index 0）。
- **理由標記**：寫成「stale-geometry / 熱插拔 / 多螢幕硬化」，**不要寫成「DPI 縮放修正」**（已推翻）。
- **風險**：低。PMv2 單螢幕下兩來源不發散，故非本次主修。

---

## 6. 風險與不確定性（誠實清單）

1. **A 的發生頻率未實測**：靜態畫面 WGC 影格節奏無 Microsoft 決定性官方聲明（佐證來自 windows-capture maintainer 討論 + DXGI duplication 類比）；移動中會自我緩解。是主因或偶發次因 **必須跑 V1/V2 才能定**。
2. **DPI 推翻的 1% 反例**：若機器被 AppCompat shim 強制 System-DPI-aware，`GetMonitorInfoW` 會回 logical 1536×864 而 cursor 仍 physical → 真發散。正常 Tauri app 不會，**V1 一行 log 即可一錘定音**。
3. **B 是「慢」還是「輸出錯」未量測**：`--no-mmproj-offload` 文件上是正確性保留（只慢），但未在此 GPU/model 實測。**V0 + V3 待測**。你的 GPU 實際 VRAM 也請用 V3 的 stderr `vision offload` 行確認（報告中「12GB」是依 `DEBUG_VRAM_LAG.md` 推定，需你確認）。
4. **C 對本案的角色**：依你「紅字」描述判定 C 非主因；但若 V0 出現 `response decode failed` 之外的「假成功」現象，需重新評估。
5. **D 的 session 推論次數未知**：500 次/session 是否對你構成困擾取決於使用強度。
6. **多螢幕假設未建立**：階段 5 價值取決於 V6。

---

## 7. 關鍵檔案路徑（供修復參考）

- `src-tauri/src/capture/screen_capture.rs`（`:55-70` retry、`:72-79` 兩來源座標、`:76` 丟棄 captured_at、`:145-197` clamp、`:277-360` GDI 路徑）
- `src-tauri/src/capture/windows_capture_pool.rs`（`:171-172` WGC 尺寸、`:214-218` contains_point、`:220-231` snapshot 無新鮮度、`:266-267` rcMonitor 寬高被算出卻丟棄）
- `src-tauri/src/capture/pipeline.rs`（`:128-153` bbox 回代 + crop_imm、`:148-149` `+1` 補償）
- `src-tauri/src/llama_runtime/supervisor.rs`（`:72-81` VRAM 16GiB 閾值、`:140-203` 週期重啟、`:155-156` threshold=0、`:318-365` 啟動參數）
- `src-tauri/src/vlm/mod.rs`（`:718-795` submit 無 guard、`:887-912` ensure_min_dimension、`:919-926` 寬鬆 parser、`:477-480` 成功寫 log+clipboard）
- `src-tauri/src/overlay.rs`（regression 視窗未改、已排除）
