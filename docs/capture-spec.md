# Capture2Text 原版 Q/W/E 行為規格（for Rust port）

Source of truth：upstream `upstream/Capture2Text/`。本文件把三個預設熱鍵、preprocess pipeline、UI 回饋的上游行為整理成 Rust 實作所需的規格。Implementation 必須達成同等行為或更佳（user 明確要求）。

## 熱鍵 → Capture Mode 對應（upstream 預設）

| Hotkey | Capture mode | upstream 函式 |
|---|---|---|
| **Win+Q** | CaptureBox（手動框選）| `MainWindow::startCaptureBox()` |
| **Win+W** | ForwardTextLineCapture（游標向前找文字行）| `MainWindow::performForwardTextLineCapture(pt)` |
| **Win+E** | TextLineCapture（游標雙向找文字行）| `MainWindow::performTextLineCapture(pt)` |
| Win+R | ReCaptureLast（本期先不做）| |
| Win+S | BubbleCapture（本期先不做）| |

來源：`Settings.cpp` L28-32。

## Mode 差異（Win+W vs Win+E）

兩者都呼叫 `PreProcess::extractTextBlock(pixs, pt_x, pt_y, lookahead, lookbehind, searchRadius)`，但 **crop 形狀** 和 **參數** 不同：

### Win+W（Forward，游標 = 行首附近）

Crop 形狀（水平書寫）：
- left = pt.x − `StartOffset`（預設 25）
- top = pt.y − `Width/2`（預設 70 / 2 = 35）
- right = pt.x + `Length`（預設 750）
- bottom = pt.y + `Width/2`

Params（`MainWindow.cpp` L446-448）：
- lookahead = `ForwardTextLineCaptureLookahead` = **14**
- lookbehind = `ForwardTextLineCaptureLookbehind` = **1**
- searchRadius = `ForwardTextLineCaptureSearchRadius` = **30**

特點：後方只給 1 行容許度（游標期望在第一個字附近）、前方最多 14 行。

### Win+E（TextLine，游標 = 行中任意）

Crop 形狀（水平書寫）：
- left = pt.x − `Length/2`（預設 1500 / 2 = 750）
- top = pt.y − `Width/2`（預設 70 / 2 = 35）
- right = pt.x + `Length/2`
- bottom = pt.y + `Width/2`

Params（`MainWindow.cpp` L573-575）：
- lookahead = `TextLineCaptureLookahead` = **14**
- lookbehind = `TextLineCaptureLookbehind` = **14**
- searchRadius = `TextLineCaptureSearchRadius` = **30**

特點：雙向對稱。

### Win+Q（CaptureBox 手動）

User 拖拉出矩形後，直接 OCR 該矩形，不呼叫 `extractTextBlock`。首期若只做 W/E 可緩做，但 user 要求 100% parity → 要實作。

## Vertical 判斷

`isVertical = languageSupportsVerticalOrientation(lang) && orientation ∈ {"Auto","Vertical"}`。

Vertical 時 crop 寬高互換（見 `MainWindow.cpp` L408-421 / L536-549）。Rust MVP 可先硬寫 horizontal，但要留介面擴充。

## 完整 preprocess pipeline（`PreProcess::extractTextBlock`，L617-735）

1. **makeGray**：`pixConvertRGBToGray(pixs, 0,0,0)`（32bpp）/ `pixConvertTo8(pixs, 0)`（其他）
2. **binarizeForNeg**：先跑一次 Otsu 得臨時 1bpp，用來判斷背景深淺
3. **pixAverageInRect**：在游標附近 40×40 框取平均值
4. **if avg > 0.5 → pixInvert**：背景深色時反白灰階影像
5. **scaleUnsharpBinarize**：
   - scale：`pixScaleGrayLI(gray, scaleFactor, scaleFactor)` — 預設 `OcrScaleFactor = 3.5f`（clamp 0.71 ~ 5.0）
   - unsharpMask：`pixUnsharpMaskingGray(pix, halfwidth=5, fract=2.5)`
   - binarize：`pixOtsuAdaptiveThreshold(pix, sx=2000, sy=2000, smoothX=0, smoothY=0, scorefract=0.0, &thresh, &bin)`
6. **erase connecting border**（依書寫方向）：
   - horizontal：`eraseConnectingBorderPixelsRight(bin, pt_x * scaleFactor)`
   - vertical：`eraseConnectingBorderPixelsBelow(bin, pt_y * scaleFactor)`
   - 作法：`pixExtractBorderConnComps(bin, 8)` → 掃每 row/col，找第一個黑點 → `pixClearInRect` 清掉該點右側/下方一條 1px 線（見 `eraseLineToRightOfPoint`/`eraseLineBelowPoint` L528-558）
7. **pixRemoveBorderConnComps(bin, 8)**：幹掉黏邊的連通塊（漫畫對話框）
8. **removeNoise**：`pixSelectBySize(pix, 3, 3, 8, L_SELECT_IF_EITHER, L_SELECT_IF_GT, nullptr)` — 保留 width 或 height > 3 的 blob
9. **BoundingTextRect::getBoundingRect(denoise, pt_x*scale, pt_y*scale, vertical, lookahead*scale, lookbehind*scale, searchRadius*scale)**
10. **pixClipRectangle(bin, boundingRect)**：從 step 5 的 binarize 結果裁出 bbox
11. **eraseFurigana**（日文時才開）
12. 後續：remove border, addBorder, setDPI → OCR

**關鍵**：bbox 存在 scaled 座標系，getter `getBoundingRect()` 除回 scaleFactor 得到原始圖座標。

## UI 回饋（upstream 框選視覺）

`CaptureBox` 物件兼作 manual drag 和 autoCapture 顯示。

### 顏色預設（`Settings.cpp` L25-26）

- `defaultCaptureBoxBackgroundColor = QColor(0, 128, 255, 60)` — **RGBA(0,128,255,60) 藍色填充**（alpha 60/255 ≈ 23.5%）
- `defaultCaptureBoxBorderColor = QColor(0, 128, 255, 255)` — **RGBA(0,128,255,255) 藍色 1px 實線邊框**

> 注意：`CaptureBox.cpp` L27/29 的 constructor 初值是紅色，但 `MainWindow::createCaptureBox/createAutoCaptureBox` 會立刻用 `Settings::getCaptureBoxBackgroundColor/BorderColor` 覆蓋成**藍色**。預設視覺是藍色。

### 三種用法

| 情境 | background | border |
|---|---|---|
| Win+Q 拖拉中 | 藍填充 RGBA(0,128,255,60) | 藍邊 RGBA(0,128,255,255) |
| Win+W/E autoCapture（顯示偵測結果）| **關閉**（`setUseBackgroundColor(false)`）| 藍邊（`setBorderColor`）|
| Settings 預覽 | 當前設定值 | 當前設定值 |

### AutoCapture 時序（`CaptureBox::autoCapture` L235-241）

```
rect = displayRect  // from bbox / scaleFactor, offset to screen coords
box.setFixedSize(rect.w + 2, rect.h + 2)
box.move(rect.x - 1, rect.y - 1)  // 邊框在 bbox 外側 1px
box.show()
timer.start(500)  // 500ms 後自動隱藏
```

Rust 實作要做到：偵測完成 → 螢幕上顯示 1px 藍邊框（無填充）500ms → 淡出。

## DPI 與座標

- `GetCursorPos` 在 DPI per-monitor-aware 程序會回「logical」座標；xcap 截圖是 physical。混用會 off-by-scale。
- 解法：用 `GetPhysicalCursorPos`（`Win32_UI_HiDpi`）取得 physical 座標，或把程序宣告為 DPI unaware（較不推薦）。
- bbox 還原時 `x / scale`、`y / scale` 要 floor；`(x+w) / scale`、`(y+h) / scale` 要 ceil，否則裁出的矩形比上游小。

## 座標回代（bbox → 螢幕座標）

```
displayRect.left   = pt.x − ptInCropRect.x + bbox.x + 1
displayRect.top    = pt.y − ptInCropRect.y + bbox.y + 1
displayRect.right  = displayRect.left + bbox.width
displayRect.bottom = displayRect.top + bbox.height
```

`+1` 是因為 bbox 在演算法內部是從 scaled 座標 floor 回原圖時偏右下 1px 補償（見 `MainWindow.cpp` L491-494）。

## 最小 OCR 尺寸

`MainWindow.h`:
- `minOcrWidth = 3`
- `minOcrHeight = 3`

bbox width/height 任一 < 3 → 放棄。

## 常數清單（Rust 實作用）

```
OCR_SCALE_FACTOR = 3.5   // clamp [0.71, 5.0]
DARK_BG_THRESHOLD = 0.5
USM_HALFWIDTH = 5
USM_FRACT = 2.5
OTSU_SX = 2000
OTSU_SY = 2000
OTSU_SMOOTH_X = 0
OTSU_SMOOTH_Y = 0
OTSU_SCOREFRACT = 0.0
REMOVE_NOISE_MIN_BLOB = 3
MIN_OCR_WIDTH = 3
MIN_OCR_HEIGHT = 3
AUTO_CAPTURE_DISPLAY_MS = 500
CAPTURE_BG_COLOR_RGBA = (0, 128, 255, 60)
CAPTURE_BORDER_COLOR_RGBA = (0, 128, 255, 255)
NEG_RECT_PROBE_SIZE = 40

// Win+W
FWD_LINE_WIDTH = 70
FWD_LINE_LENGTH = 750
FWD_LINE_START_OFFSET = 25
FWD_LINE_LOOKAHEAD = 14
FWD_LINE_LOOKBEHIND = 1
FWD_LINE_SEARCH_RADIUS = 30

// Win+E
LINE_WIDTH = 70
LINE_LENGTH = 1500
LINE_LOOKAHEAD = 14
LINE_LOOKBEHIND = 14
LINE_SEARCH_RADIUS = 30
```

## 尚未 port 的上游函式（Rust 要補）

| 上游函式 | 用途 | leptonica-sys 對應 FFI |
|---|---|---|
| `pixConvertRGBToGray` | 32bpp → gray | `pixConvertRGBToGray` |
| `pixConvert24To32` | 24bpp fixup | `pixConvert24To32` |
| `pixScaleGrayLI` | gray scale（雙線性）| `pixScaleGrayLI` |
| `pixUnsharpMaskingGray` | 銳化 | `pixUnsharpMaskingGray` |
| `pixOtsuAdaptiveThreshold` | 二值化 | `pixOtsuAdaptiveThreshold` |
| `pixExtractBorderConnComps` | 邊緣連通塊 | `pixExtractBorderConnComps` |
| `pixClearInRect` | 清除矩形內像素 | `pixClearInRect` |
| `pixAverageInRect` | 矩形平均 | `pixAverageInRect` |
| `pixInvert` | 影像反白 | `pixInvert` |
| `pixSelectBySize` | 依 blob 尺寸過濾 | `pixSelectBySize` |
| `pixClipRectangle` | 裁切矩形 | `pixClipRectangle` |

`Pix` wrapper 要擴充對應 method。

## 本期範圍（Win+Q/W/E parity）

1. Win+Q：手動 drag box + OCR（後期處理）
2. Win+W：forward line extract + overlay
3. Win+E：bidirectional line extract + overlay
4. 以上皆用上面常數 & pipeline & 藍色 overlay
5. Vertical 先硬寫 horizontal，預留 enum 接口

Tesseract OCR 本身暫不接，先確認「偵測正確 → 畫出正確藍框」這段完全對得上上游。

---

# Drift Analysis（v1 vs upstream）

User 要求：「同等的體驗 甚至 更好、更快、更準」。以下是深挖對比結果。

## 已對齊（verified matching）

| 項目 | 狀態 |
|---|---|
| W params（25/750/70/14/1/30）| ✓ `capture/params.rs` |
| E params（1500/70/14/14/30）| ✓ `capture/params.rs` |
| OCR_SCALE_FACTOR=3.5 | ✓ `capture/preprocess.rs` |
| Preprocess pipeline 順序與常數 | ✓ `capture/preprocess.rs` |
| BoundingTextRect 演算法 | ✓ `leptonica/bounding_rect.rs` |
| 藍邊框色 RGBA(0,128,255,255)| ✓ `public/overlay.html` |
| 500ms autoCapture 顯示 | ✓ `overlay.rs::show` |
| Overlay 尺寸 = bbox+2、位置 = bbox-1 | ✓ `overlay.rs::show` |
| `GetPhysicalCursorPos` 取 physical 座標 | ✓ `hotkey/keyboard_hook.rs` |
| WH_KEYBOARD_LL 攔 Win+Q/W/E + Ctrl-tap 抑制 Start menu | ✓（memory 硬規則）|

## 確認 drift（v2 要修）

### D1. 每次熱鍵截兩次圖（effective latency ×2）

現況：`screenshot::worker_loop` 先 `capture_and_save(kind)` 存 PNG、再 `pipeline::run_for_event(event)` 又呼叫一次 `capture_primary_monitor()`。兩次都是 xcap 整螢幕 50–150ms。

上游：一次 `screen->grabWindow(0, cropRect)`（crop-only）。

### D2. 整螢幕截圖 vs crop-only（xcap vs BitBlt）

現況：xcap 抓整張 monitor（1920×1080+）再 software crop。
上游：`QScreen::grabWindow(0, x, y, w, h)` 內部用 GDI BitBlt，只 blit 小矩形（W mode ≈ 775×70）。

延遲差：整螢幕 50–150ms vs crop 5–15ms，**約 10× 差距**。

### D3. 多螢幕：固定抓 primary，不是游標所在螢幕

現況：`capture_primary_monitor()` 用 `Monitor::all()` + `is_primary()` 固定挑 primary。
上游：`QDesktopWidget::screenGeometry(pt)` 取 **游標所在** 的螢幕。

後果：game machine 上游標在第二螢幕按 Win+W → 抓了 primary 的像素、或 crop 座標越界直接失敗（取決於 layout）。

### D4. 座標回代缺 `+1` 偏移

現況 `pipeline.rs::run_for_event`：
```rust
x: capture.monitor_x + crop.x + result.bbox_unscaled.x,
y: capture.monitor_y + crop.y + result.bbox_unscaled.y,
```

上游 `MainWindow.cpp` L491–494：
```cpp
displayRect.setRect(
    pt.x() - ptInCropRect.x() + boundingBox.x() + 1,
    pt.y() - ptInCropRect.y() + boundingBox.y() + 1,
    boundingBox.width(), boundingBox.height());
```

`pt.x() - ptInCropRect.x()` ≡ `crop.x`（絕對螢幕座標）；所以上游 = `crop.x + bbox.x + 1`，我們漏了 `+1`。視覺上藍框會整體右下偏移 1px。

### D5. Overlay stack：Tauri WebviewWindow vs native HWND

現況：`WebviewWindowBuilder` 建立的 window 背後是 WebView2。`set_position` / `set_size` / `show` / `hide` 每個都是 IPC roundtrip；CSS 1px border 在 HiDPI + transparent 下右下邊緣被 device-pixel snap 切掉（user 實測回報）。
上游：Qt QWidget（薄包裝在 HWND 上）。`setFixedSize` + `move` + `show` 三個 Win32 calls，繪圖 `paintEvent` 直接 GDI。

延遲差：WebView2 首次 show 50–200ms（cold）+ subsequent 50ms / native HWND 5ms。

### D6. 缺 minOcrWidth/Height OR-check（unscaled）

現況 `preprocess.rs` L94：
```rust
if bbox_scaled.w < 3 && bbox_scaled.h < 3 { return Ok(None); }
```
（AND，在 scaled 座標系）— 這 **對齊上游 `PreProcess::extractTextBlock` 內部的自我檢查**。

但上游 `MainWindow::performTextLineCapture` 之後還有一層：
```cpp
if (boundingBox.width() < minOcrWidth || boundingBox.height() < minOcrHeight)
    return; // minOcrWidth = minOcrHeight = 3
```
（OR，在 unscaled / 原圖座標系）。我們缺這層外層檢查。

效果：`bbox_scaled=(0,0,10,2)`（scaled 下 w=10 過關），unscaled 回推成 `(0,0,3,1)` → 應該被捨棄，但我們會畫一個 1px 高的藍框。

### D7. Vertical text 沒接

現況 `pipeline.rs` L61：`vertical: false`（hardcoded）。
上游：`languageSupportsVerticalOrientation(lang) && orientation ∈ {Auto, Vertical}`。

MVP 不做，但 ModeProfile 要能轉 `vertical` flag，preprocess 已支援。

### D8. First-word 模式（W）沒接

上游 `FirstWordCapture=false` 預設。true 時 W 只抓第一個字。

本期暫不做。

## 不改動項目（守 memory 硬規則）

- **不能** 改 WH_KEYBOARD_LL → RegisterHotKey（雖然 RegisterHotKey 天生抑制 Start menu 可省 Ctrl-tap），理由：RegisterHotKey 在遊戲全螢幕、UAC elevated window、某些 overlay 下收不到鍵，過去 5 次失敗都卡這。WH_KEYBOARD_LL 是 hard rule。
- **不能** 在 hook_proc 內做 I/O（LowLevelHooksTimeout 300ms 會讓 `LRESULT(1)` 靜默失效 → Win key 漏出去 → Start menu 彈出）。hook 只做 key state 讀取 + channel enqueue。

---

# STEP 3A-2 v2 架構（一次到位）

目標：v2 實測 hotkey → frame visible **≤ 上游 +10%**（粗估 ≤70–110ms），視覺 4 邊都在，多螢幕正確。

## v2 資料流

```
[hook thread] WH_KEYBOARD_LL hook_proc
    → 讀 GetPhysicalCursorPos
    → try_send(HotkeyEvent) to worker
    → SendInput Ctrl-tap（抑制 Start menu）
    → LRESULT(1)

[capture-worker thread] worker_loop
    → Monitor Monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST)  [D3 fix]
    → 算 crop rect (profile_for + clamp to Monitor rect)
    → HDC screenDC = GetDC(NULL)
      HDC memDC = CreateCompatibleDC(screenDC)
      HBITMAP bmp = CreateCompatibleBitmap(screenDC, crop.w, crop.h)
      SelectObject(memDC, bmp)
      BitBlt(memDC, 0, 0, crop.w, crop.h, screenDC, crop.x, crop.y, SRCCOPY)  [D1+D2 fix]
    → GetDIBits → RGBA buffer → Pix::from_raw（零 PNG roundtrip）
    → extract_text_block → ExtractResult
    → 若 bbox_unscaled.w<3 || bbox_unscaled.h<3 → skip（minOcr check，D6 fix）
    → screen_x = monitor.x + crop.x + bbox_unscaled.x + 1  [D4 fix]
      screen_y = monitor.y + crop.y + bbox_unscaled.y + 1
    → overlay::show(screen_x, screen_y, w, h)

[overlay — 永駐 Win32 layered window]
    → SetWindowPos(HWND_TOPMOST, x-1, y-1, w+2, h+2, SWP_NOACTIVATE)
    → UpdateLayeredWindow（預先 render 好的 1px 藍邊 + 透明填充 HBITMAP）
    → ShowWindow(SW_SHOWNOACTIVATE)
    → spawn 500ms timer → ShowWindow(SW_HIDE)（AtomicU64 latest-wins 保留）
```

## v2 實作順序（Codex 共識版 — 風險面積最小化）

採用分階段 commit，每階段都要能 compile + 保留前一階段行為，用來定位 regression 與量測 latency 改善幅度。

### Commit 1 — correctness/perf fixes（不動架構）
- D1：`worker_loop` **完全移除** `capture_and_save` 呼叫（含函式本體，debug save 留到 Commit 2 用 shared buffer 做）
- D3：`capture_primary_monitor` 改成 `xcap::Monitor::from_point(cursor.x, cursor.y)` 取游標所在螢幕（xcap 內部就是 `MonitorFromPoint(POINT, MONITOR_DEFAULTTONULL)`）
- D4：`run_for_event` 回傳 bbox 前，`screen_x += 1, screen_y += 1`，`w/h 不加`（外擴 -1/+2 由 overlay 負責，不要重複）
- D6：`run_for_event` 在 `extract_text_block` 回 `Some` 後，加 `if bbox_unscaled.w < 3 || bbox_unscaled.h < 3 { return Ok(None); }`（MainWindow minOcr 檢查，unscaled 座標系）

### Commit 2 — `screen_capture` 抽象
- 新檔 `src-tauri/src/capture/screen_capture.rs`，API：
  ```rust
  pub struct MonitorRect { pub x: i32, pub y: i32, pub w: i32, pub h: i32 }
  pub struct CaptureOutput { pub monitor: MonitorRect, pub image: RgbaImage, pub crop_x: i32, pub crop_y: i32 }
  pub fn capture_at_cursor(cursor: CursorPoint, crop: ProfileCropRequest) -> io::Result<CaptureOutput>
  ```
- 內部呼叫 `Monitor::from_point` + `monitor.capture_region(crop_x, crop_y, crop_w, crop_h)`（xcap 0.9.4 GDI 路徑就是 `BitBlt(desktopDC → memDC) + GetDIBits`，等於上游 `QScreen::grabWindow` 的路）
- `pipeline.rs` 改呼叫這個 API
- 同時把 C2T_DEBUG_SAVE=1 的 debug save 接回來（存 shared buffer，不再額外抓圖）
- **此階段做第一次 latency benchmark 對比上游**

### Commit 3 — Win32 layered window overlay（取代 WebviewWindow）
- 重寫 `src-tauri/src/overlay.rs`，API 改為：
  ```rust
  pub fn init() -> io::Result<()>           // 不再需 AppHandle
  pub fn show(bbox: BoundingBoxScreen)
  pub fn shutdown()                          // optional
  ```
- 獨立 `std::thread::spawn` 跑 overlay GUI thread，持有 HWND + message pump（`GetMessageW` 迴圈）
- worker → overlay 走 mpsc channel（`show(Rect)` / `hide` / `exit`）；channel 接到後 `PostMessage(hwnd, WM_APP_*, …)`，overlay thread 在 `WndProc` 處理
- 單例資源：`HWND`、`HDC(mem)`、`HBITMAP(DIB section)`、`bits_ptr`、`cur_w/cur_h`、`AtomicU64 generation`
- `show` 流程：
  1. 若 `bbox.w+2 != cur_w || bbox.h+2 != cur_h` → recreate DIB
     - `SelectObject(hdc_mem, old_bitmap)` 還原 → `DeleteObject(hbm)` → 新 `CreateDIBSection` → 新 `SelectObject` 記下舊的 bitmap handle
  2. 清空 bits（`memset` alpha=0）
  3. 畫 1px RGBA(0,128,255,255) 邊（寫 DIB bits 或 `FrameRect`）
  4. `SetWindowPos(hwnd, HWND_TOPMOST, bbox.x-1, bbox.y-1, bbox.w+2, bbox.h+2, SWP_NOACTIVATE | SWP_NOZORDER_if_first_time_only)`
  5. `UpdateLayeredWindow(hwnd, nullptr, nullptr, &size, hdc_mem, &POINT(0,0), 0, &BLENDFUNCTION, ULW_ALPHA)`
  6. `ShowWindow(hwnd, SW_SHOWNOACTIVATE)`
  7. `fetch_add` generation，500ms 後若 still latest 再 `ShowWindow(SW_HIDE)`
- 窗口 style：`WS_POPUP`；ex-style：`WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`
- `WM_DESTROY`：`SelectObject(hdc_mem, old_bitmap) → DeleteObject(hbm) → DeleteDC(hdc_mem) → DestroyWindow`

### Commit 4（可能跳過）— 自寫 BitBlt + luma Pix
只有 Commit 2 benchmark 仍不達標才做：
- `capture/screen_capture.rs` 增加第二 backend
- `BitBlt(desktopDC → memDC, crop_rect, SRCCOPY)` + `GetDIBits → BGRA buffer`
- 拷貝時直接做 `luma = 0.299R + 0.587G + 0.114B` → `pixCreate(w, h, 8)` + `pixGetData` 寫 row，省掉後續 `make_gray` 的 RGB→gray 轉換
- 不走 `pixSetData` 接管外部 buffer（Leptonica allocator 責任太危險，per Codex）

### Commit 5 — cleanup
- 刪除 `public/overlay.html`
- 刪除 `capture/screenshot.rs::capture_and_save` 殘留
- 清 `tauri.conf.json` 內跟 overlay webview 相關的設定
- 最終 full regression pass

## 驗收標準（v2）

| 項目 | 目標 |
|---|---|
| Win+W/E 冷啟動（應用第一次按）| ≤ 150ms hotkey→frame |
| Win+W/E 熱迴圈（第二次以後）| ≤ 70ms hotkey→frame |
| 藍框 4 邊齊全（HiDPI）| ✓ |
| 主螢幕以外的多螢幕能用 | ✓ |
| 同一張 PNG 輸入，bbox 與上游 pixel-exact | ±1px 內 |
| minOcr 過濾 2px 以下雜訊 | ✓ |
| 連續按 5 次不 crash、沒有殭屍 overlay | ✓ |

延遲量測方式：錄螢幕 240fps（每 frame 4.17ms）→ 看按鍵按下那 frame 到藍框出現 frame 的距離。上游也量同樣指標做 baseline。

## v2 風險

| 風險 | 緩解 |
|---|---|
| Win32 layered window 在 WDDM 下偶發不畫 | `SetLayeredWindowAttributes(LWA_COLORKEY)` fallback |
| BitBlt 在受保護視窗（DRM、某些 UWP）回黑 | v1 本來就這樣，非回歸；日後 `PrintWindow(PW_RENDERFULLCONTENT)` 備案 |
| 多螢幕 DPI 混合 | `SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2)`（Tauri 可能已設）+ `GetDpiForMonitor` |
| Codex 一次改動過大 | 拆 5 個小 commit：螢幕抓圖抽象 / 覆蓋層 Win32 / coord+1 / minOcr / dedup capture_and_save |
