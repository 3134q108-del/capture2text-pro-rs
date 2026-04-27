# T38 · Output tab 內容

## 目標

實作 Output tab：
1. ☑ 儲存到剪貼簿（連動 window_state.save_to_clipboard）
2. ☑ 顯示翻譯彈窗（連動 window_state.popup_show_enabled）
3. ☐ 將擷取記錄到檔案（log_enabled） + 檔案位置顯示

## 鎖死（MUST）

### 1. `src-tauri/src/window_state.rs` 加欄位

```rust
#[serde(default = "default_log_enabled")]
pub log_enabled: bool,
#[serde(default = "default_log_file_path")]
pub log_file_path: String,

fn default_log_enabled() -> bool { false }
fn default_log_file_path() -> String {
    use std::path::PathBuf;
    let base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("Capture2TextPro").join("captures.log")
        .to_string_lossy().to_string()
}
```

同樣更新 `Default for WindowState`。

### 2. `src-tauri/src/window_state.rs` 新 setter

```rust
pub fn set_log_enabled(v: bool) { update(|s| s.log_enabled = v); }
pub fn set_log_file_path(v: String) { update(|s| s.log_file_path = v); }
```

### 3. `src-tauri/src/commands/result_window.rs` 新 commands

```rust
#[tauri::command]
pub fn set_log_enabled(value: bool) -> Result<(), String> {
    window_state::set_log_enabled(value); Ok(())
}

#[tauri::command]
pub fn set_log_file_path(value: String) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err("log file path cannot be empty".into());
    }
    window_state::set_log_file_path(value); Ok(())
}
```

### 4. 新檔 `src-tauri/src/capture/log.rs`

```rust
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use chrono::Local;

pub fn append_capture(original: &str, translated: &str) {
    let state = crate::window_state::get();
    if !state.log_enabled { return; }
    let path = &state.log_file_path;
    if path.is_empty() { return; }
    if let Some(parent) = Path::new(path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let ts = Local::now().format("%Y-%m-%d %H:%M:%S");
    let line = format!("{}\t{}\t{}\n",
        ts,
        original.replace('\t', " ").replace('\n', " "),
        translated.replace('\t', " ").replace('\n', " "));
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = f.write_all(line.as_bytes());
    }
}
```

### 5. `src-tauri/src/capture/mod.rs` 宣告 `pub mod log;`

（如果 capture/mod.rs 已存在類似宣告 pattern；Codex 先讀確認）

### 6. VLM event pipeline 掛鈎

`src-tauri/src/vlm/mod.rs` 的 `emit_vlm_event`（或類似 final emit 點，Codex 找實際位置）：
- 在 `status == "success"` 分支 **最後**加：
  ```rust
  crate::capture::log::append_capture(&original, &translated);
  ```
- 在已有 `prefetch(&original, &voice_zh)` / `prefetch(&translated, &voice_en)` 邏輯之前或之後都可（不影響 TTS）

如果 vlm 層不方便 import capture 模組（cross-mod），改放 `commands/result_window.rs` 或直接在 vlm/mod.rs emit 完 event 的 listener side 處理；**最簡單**：vlm/mod.rs 直接呼叫 `crate::capture::log::append_capture(...)`。

### 7. `src-tauri/src/lib.rs`

invoke_handler 註冊 `set_log_enabled` + `set_log_file_path`。

### 8. `src/settings/tabs/OutputTab.tsx` 完整實作

```tsx
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

type WindowState = {
  save_to_clipboard: boolean;
  popup_show_enabled: boolean;
  log_enabled: boolean;
  log_file_path: string;
};

export default function OutputTab() {
  const [saveClipboard, setSaveClipboard] = useState<boolean>(true);
  const [showPopup, setShowPopup] = useState<boolean>(true);
  const [logEnabled, setLogEnabled] = useState<boolean>(false);
  const [logPath, setLogPath] = useState<string>("");
  const [statusMsg, setStatusMsg] = useState<string>("");

  useEffect(() => { void refresh(); }, []);

  async function refresh() {
    try {
      const ws = await invoke<WindowState>("get_window_state");
      setSaveClipboard(ws.save_to_clipboard);
      setShowPopup(ws.popup_show_enabled);
      setLogEnabled(ws.log_enabled);
      setLogPath(ws.log_file_path);
    } catch (err) { setStatusMsg(String(err)); }
  }

  async function updateSaveClipboard(v: boolean) {
    setSaveClipboard(v);
    try { await invoke("set_save_to_clipboard", { value: v }); }
    catch (err) { setStatusMsg(String(err)); }
  }
  async function updateShowPopup(v: boolean) {
    setShowPopup(v);
    try { await invoke("set_popup_show_enabled", { value: v }); }
    catch (err) { setStatusMsg(String(err)); }
  }
  async function updateLogEnabled(v: boolean) {
    setLogEnabled(v);
    try { await invoke("set_log_enabled", { value: v }); }
    catch (err) { setStatusMsg(String(err)); }
  }
  async function updateLogPath(v: string) {
    setLogPath(v);
    try { await invoke("set_log_file_path", { value: v }); }
    catch (err) { setStatusMsg(String(err)); }
  }

  return (
    <div className="settings-translate-root">
      <section className="settings-section">
        <label className="settings-checkbox">
          <input type="checkbox" checked={saveClipboard} onChange={e => updateSaveClipboard(e.target.checked)} />
          儲存原文到剪貼簿
        </label>
      </section>

      <section className="settings-section">
        <label className="settings-checkbox">
          <input type="checkbox" checked={showPopup} onChange={e => updateShowPopup(e.target.checked)} />
          截圖後顯示翻譯彈窗
        </label>
      </section>

      <section className="settings-section">
        <label className="settings-checkbox">
          <input type="checkbox" checked={logEnabled} onChange={e => updateLogEnabled(e.target.checked)} />
          將每次擷取記錄到檔案
        </label>
        <div style={{ display: "flex", flexDirection: "column", gap: 6, marginTop: 6 }}>
          <label>
            記錄檔位置
            <input type="text" value={logPath} onChange={e => updateLogPath(e.target.value)} />
          </label>
          <div style={{ fontSize: 12, color: "var(--c2t-text-muted)" }}>
            格式：{"時間戳\\t原文\\t譯文\\n"}（啟用後每次截圖成功時附加一行）
          </div>
        </div>
      </section>

      {statusMsg && <div className="settings-status">{statusMsg}</div>}
    </div>
  );
}
```

## 禁動

- **不動** 其他 tab 檔
- **不動** Popup (ResultView)
- **不動** Clipboard 實際寫入流程（T32 做）

## 驗證

- `cargo check` 通過
- `npm build` 通過
- **不需**手測

## 回報

```
=== T38 套改結果 ===
- window_state.rs 擴 log_enabled/log_file_path + setters
- commands/result_window.rs 新 2 commands
- capture/log.rs 新檔
- capture/mod.rs 宣告
- vlm/mod.rs emit_vlm_event 掛 append_capture
- lib.rs 註冊 commands
- OutputTab.tsx 完整重寫
- cargo check: <結果>
- npm build: <結果>
- UTF-8 NoBOM

VERDICT: APPROVED
```

**直接套改**，不需要 diff 提案。UTF-8 NoBOM。
