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
