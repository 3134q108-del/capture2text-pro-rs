# T40 · Tray menu 重寫對齊 Stage 7 最終結構

## 背景

T36a 已擴 output_lang 為 5 語。Tray menu 原本只有 2 語 submenu + `顯示設定... / 結束` 兩項 top item，要對齊最終結構重寫：

```
設定...
─────────────
顯示彈窗         [✓]
儲存到剪貼簿     [✓]
─────────────
輸出語言  ▸      ⦿ 繁體中文
                 ⦾ 簡體中文
                 ⦾ 英文
                 ⦾ 日文
                 ⦾ 韓文
─────────────
關於...
離開
```

## 目標

重寫 `src-tauri/src/tray.rs::install` 成上述結構，所有 menu 互動連動對應 state。

## 鎖死（MUST）

### Menu 結構（由上到下）

1. **設定...** → `show_settings` — 呼叫 `show_settings_window(app)`
2. 分隔線 `PredefinedMenuItem::separator`
3. **顯示彈窗** CheckMenuItem → `toggle_show_popup`
   - 初始 checked = `window_state::get().popup_show_enabled`
   - 點擊：
     - toggle window_state.popup_show_enabled via `window_state::set_popup_show_enabled(new_val)`
     - `set_checked(new_val)`
4. **儲存到剪貼簿** CheckMenuItem → `toggle_save_clipboard`
   - 初始 checked = `window_state::get().save_to_clipboard`
   - 點擊：
     - toggle via `window_state::set_save_to_clipboard(new_val)`
     - `set_checked(new_val)`
5. 分隔線
6. **輸出語言** Submenu 含 5 個 CheckMenuItem（一次只一個 checked）
   - id：`lang_zh_tw` / `lang_zh_cn` / `lang_en_us` / `lang_ja_jp` / `lang_ko_kr`
   - label：`繁體中文` / `簡體中文` / `英文` / `日文` / `韓文`
   - 初始 checked：`output_lang::current() == "zh-TW"` 等（對應 5 個 code）
   - 點擊某 lang：
     - `output_lang::set("zh-TW")` 等（對應 code）
     - 5 個 item：選中者 `set_checked(true)`，其他 4 個 `set_checked(false)`
7. 分隔線
8. **關於...** → `show_about` — 呼叫 `show_settings_window(app)` + emit `settings-navigate:about` event 到 `settings` window（讓 SettingsView 收到切換到 About tab；T28 shell 還沒 listen 這 event，但先 emit，T39 會接）
9. **離開** → `quit` — `app.exit(0)`

### 分隔線用法

```rust
use tauri::menu::PredefinedMenuItem;
let sep = PredefinedMenuItem::separator(app)?;
```

### Clone 策略

所有 CheckMenuItem 在 closure 裡要 `set_checked` 的 → 在 closure 外 `.clone()` 進 closure（沿用現有 tray.rs 的 `let lang_zh_item = lang_zh.clone();` pattern）。

### Emit event 寫法

```rust
use tauri::Emitter;
let _ = app.emit_to("settings", "settings-navigate", "about");
```

（emit payload 用 string `"about"`，T39 實作 listener 再決定 parse 方式；T28 shell 現在會靜靜忽略此 event）

## 禁動

- **不動** 前端任何檔（T36b 會處理 TranslateTab radio 同步）
- **不動** `window_state.rs`（T28 已暴露 setters，直接用）
- **不動** `output_lang.rs`（T36a 已擴 5 語）
- **不動** `lib.rs`（tray::install 簽名不變）
- **不改** `show_menu_on_left_click(false)` 等 builder 行為

## 風險點

1. **`show_about` handler 裡 emit event 給 settings window**：若 settings window 還沒建起來（第一次開啟前），emit_to 會失敗 或 payload 丟失。做法：先 call `show_settings_window(app)` 確保建立並 show，再 emit。`show_settings_window` 會走 `ensure_webview_window` 重建流程。但 emit_to 在 window 建立後的下一個 tick 才能接到 → 解法：`show_settings_window` 裡的 window 是同步建的，emit_to 應該能送到；若 `show_settings_window` 回 Err 就跳過 emit。

2. **Tray checkbox 跟 Settings / Popup checkbox 不會同步**（T41 的範圍）：這輪只做 tray 單向寫入，Settings 視窗裡的 checkbox（T38 會加）目前還沒有，不用處理同步。未來 T41 會加 `window-state-changed` event 廣播。

3. **output_lang::current() 不一定命中 5 語 code**（sanitize 已保障，但舊檔可能還沒 persist 升級）：使用 match 預設值 `"zh-TW"`。

4. **現有 `use crate::output_lang;` 已在 tray.rs**，其他 import 視需要加：
   - `tauri::Emitter` for emit_to
   - `tauri::menu::PredefinedMenuItem` for separator
   - `crate::window_state`

## 驗證

- `cargo check`（src-tauri/）應通過
- **不需**跑 `npm build`

## 回報格式

```
=== T40 套改結果 ===
- tray.rs 重寫：新結構 Settings / Show Popup / Save Clipboard / Output Lang (5) / About / Exit
- cargo check: <結果>
- UTF-8 NoBOM 驗證: BOM=False

VERDICT: APPROVED
```

**直接套改，不需要先給 diff 提案**（機械性 menu 重組）。全部 UTF-8 NoBOM。
