# T32 + T41 · Clipboard pipeline + window-state-changed cross-window sync

## 目標

合併做兩件相關事：
1. **T32**：新 `clipboard` 模組（arboard），在 VLM success 依 state 決定寫 clipboard；Popup OK 按鈕也觸發
2. **T41**：每當 `window_state` 某欄位改變（tray / settings / popup 任一側觸發）→ emit `window-state-changed` event（payload = 完整 WindowState snapshot）→ 其他 window 更新 local UI

## 鎖死（MUST）

### 1. Cargo.toml 加 `arboard = "3"`

### 2. 新檔 `src-tauri/src/clipboard.rs`

```rust
use crate::window_state;

pub fn write_capture(original: &str, translated: &str) {
    let state = window_state::get();
    if !state.save_to_clipboard { return; }

    let text = if state.translate_append_to_clipboard && !translated.is_empty() {
        let sep = separator_char(&state.translate_separator);
        format!("{original}{sep}{translated}")
    } else {
        original.to_string()
    };

    match arboard::Clipboard::new() {
        Ok(mut cb) => {
            if let Err(err) = cb.set_text(text.clone()) {
                eprintln!("[clipboard] set_text failed: {err}");
            } else {
                eprintln!("[clipboard] wrote {} chars", text.len());
            }
        }
        Err(err) => {
            eprintln!("[clipboard] init failed: {err}");
        }
    }
}

fn separator_char(key: &str) -> &'static str {
    match key {
        "Tab" => "\t",
        "LineBreak" => "\n",
        "Comma" => ",",
        "Semicolon" => ";",
        "Pipe" => "|",
        _ => " ",  // Space 或未知
    }
}
```

### 3. `src-tauri/src/lib.rs` 宣告 `mod clipboard;`

### 4. VLM success 掛鈎

`src-tauri/src/vlm/mod.rs` 的 `emit_vlm_event` success 分支（已掛 `capture::log::append_capture` by T38），**加一行**：
```rust
crate::clipboard::write_capture(&payload.original, &payload.translated);
```

位置：在 `append_capture` 之前或之後都可。

### 5. Popup OK 按鈕觸發 clipboard

`src/result/ResultView.tsx` 的 OK 按鈕（`handleOk` 或類似 handler）：
- 保留關窗邏輯
- **加**：先 `await invoke("write_popup_clipboard")`，再關窗

新 command `write_popup_clipboard`（放 `commands/result_window.rs`）：
```rust
#[tauri::command]
pub fn write_popup_clipboard() -> Result<(), String> {
    let snapshot = crate::vlm::state::snapshot();
    if let Some(s) = snapshot {
        crate::clipboard::write_capture(&s.original, &s.translated);
    }
    Ok(())
}
```

### 6. T41: `window-state-changed` event broadcasting

核心概念：
- `window_state::update(mutator)` helper 在寫入後 emit broadcast
- 問題：`window_state` 模組不持有 AppHandle → 無法直接 emit
- 解法：用全域 `OnceLock<AppHandle>` 在 lib.rs setup 時儲存，其他地方 clone 用

**新 `src-tauri/src/app_handle.rs`**（新檔）：
```rust
use std::sync::OnceLock;
use tauri::AppHandle;

static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

pub fn set(handle: AppHandle) {
    let _ = APP_HANDLE.set(handle);
}

pub fn get() -> Option<AppHandle> {
    APP_HANDLE.get().cloned()
}
```

`src-tauri/src/lib.rs`：
- `mod app_handle;`
- setup 中：`crate::app_handle::set(app.handle().clone());`（在其他 init 之前）

**修改 `src-tauri/src/window_state.rs::update`**：
在 persist 成功後加：
```rust
fn update(mutator: impl FnOnce(&mut WindowState)) {
    let slot = WINDOW_STATE.get_or_init(|| Mutex::new(load_or_default()));
    let snapshot = if let Ok(mut guard) = slot.lock() {
        mutator(&mut guard);
        persist_best_effort(&guard);
        Some(guard.clone())
    } else { None };

    if let Some(snap) = snapshot {
        if let Some(app) = crate::app_handle::get() {
            use tauri::Emitter;
            let _ = app.emit("window-state-changed", &snap);
        }
    }
}
```

同樣 `output_lang::set` 也應該 broadcast（但 payload 不同，先用 `output-language-changed` 獨立 event）：
```rust
// output_lang.rs set 內 persist 成功後
if let Some(app) = crate::app_handle::get() {
    use tauri::Emitter;
    let _ = app.emit("output-language-changed", &next);
}
```

### 7. Tray 接收 window-state-changed 更新 CheckMenuItem

`src-tauri/src/tray.rs` 的 `install` 除了原本 setup，還要：
- 在 TrayIconBuilder 建立後，**綁 listener** 到 `window-state-changed` event，在 callback 裡更新 checkbox：

```rust
// 在 install 函式最末，_tray 之後：
let show_popup_clone = show_popup_item.clone();
let save_clip_clone = save_clipboard_item.clone();
let lang_items = [
    ("zh-TW", lang_zh_tw_item.clone()),
    ("zh-CN", lang_zh_cn_item.clone()),
    ("en-US", lang_en_us_item.clone()),
    ("ja-JP", lang_ja_jp_item.clone()),
    ("ko-KR", lang_ko_kr_item.clone()),
];

app.listen("window-state-changed", move |event| {
    if let Ok(state) = serde_json::from_str::<crate::window_state::WindowState>(event.payload()) {
        let _ = show_popup_clone.set_checked(state.popup_show_enabled);
        let _ = save_clip_clone.set_checked(state.save_to_clipboard);
    }
});

app.listen("output-language-changed", move |event| {
    let lang: String = serde_json::from_str(event.payload()).unwrap_or_default();
    for (code, item) in &lang_items {
        let _ = item.set_checked(code == &lang.as_str());
    }
});
```

**注意**：`app.listen` 需要 `use tauri::Listener;` trait。

### 8. React Settings 各 tab 接收 `window-state-changed` 自動更新

在 TranslateTab / SpeechTab / OutputTab 的 useEffect 內加：

```tsx
import { listen } from "@tauri-apps/api/event";

useEffect(() => {
  void refresh();
  const stateChangedPromise = listen<WindowState>("window-state-changed", (event) => {
    // 更新本 tab 相關 state 從 event.payload
    // 這是被動同步（避免重抓）
    ...
  });
  const langChangedPromise = listen<string>("output-language-changed", (event) => {
    setOutputLang(normalizeLang(event.payload));  // TranslateTab only
  });
  return () => {
    stateChangedPromise.then(off => off());
    langChangedPromise.then(off => off());
  };
}, []);
```

### 9. Popup ResultView 接收 window-state-changed 更新 topmost / 字型

`src/result/ResultView.tsx` 已有 state for topmost/font。加 listener：
```tsx
const wsPromise = listen<WindowState>("window-state-changed", (event) => {
  const ws = event.payload;
  setTopmost(ws.popup_topmost);
  setFont(ws.popup_font);
});
```
（Codex 依 ResultView 現有 state shape 調整）

## 禁動

- **不動** VLM 業務邏輯（只加 clipboard + log）
- **不動** output_lang 5 語邏輯
- **不動** 既有 ResultView 的 Speak / Retranslate 流程
- **不改** hotkey module

## 驗證

- `cargo check` + `cargo build`
- `npm build`

## 風險

1. **tray 的 `app.listen` 在 install 內註冊**：install 可能只在 setup 執行一次，綁到 closure 的 `show_popup_clone` 等 Arc / clone 是否 safe？由 Codex 實際跑 cargo check 驗證。
2. **vlm emit + clipboard write 的 race**：clipboard write 是 blocking，在 vlm thread 內呼叫，可能 block event emit 幾百 ms。接受這個延遲。
3. **window-state-changed payload serialize**：WindowState 已 derive Serialize。emit 應 ok。

## 回報

```
=== T32+T41 套改結果 ===
- Cargo.toml 加 arboard
- clipboard.rs 新檔
- app_handle.rs 新檔
- lib.rs 宣告 + setup 存 app_handle
- vlm/mod.rs emit_vlm_event 加 clipboard::write_capture
- window_state.rs update 加 emit broadcast
- output_lang.rs set 加 emit broadcast
- tray.rs 加 2 個 listen
- commands/result_window.rs 加 write_popup_clipboard command
- lib.rs 註冊 write_popup_clipboard
- ResultView.tsx OK 按鈕加 write_popup_clipboard + window-state-changed listener
- TranslateTab.tsx / SpeechTab.tsx / OutputTab.tsx 加 listener
- cargo check: <結果>
- npm build: <結果>
- UTF-8 NoBOM

VERDICT: APPROVED
```

**直接套改**。UTF-8 NoBOM。
