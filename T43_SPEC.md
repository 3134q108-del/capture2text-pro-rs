# T43 · Clipboard mode 4 選 radio + Tray 情境 submenu

## 目標

1. **Output tab 擴「儲存到剪貼簿」**：4 選 radio
   - 不複製
   - 只複製原文
   - 只複製譯文
   - 複製原文+譯文（後面跟分隔符號 dropdown）
2. **Translate tab 移除**：「附加翻譯」checkbox + 「分隔符號」dropdown（搬到 Output tab）
3. **Tray「儲存到剪貼簿」**：從 checkbox 改成 submenu 4 radio（同 Output tab）
4. **Tray 加「情境」submenu**：比照「輸出語言」submenu，radio 互斥，列所有 scenarios（內建 + 使用者）
5. Tray scenarios submenu 在 app 啟動時 snapshot；Settings 改 scenarios 時 emit event，tray listen 後更新 checked 狀態（**不重建 submenu 項目**，只 sync checked）
6. **Speech tab slider 垂直排列**（音量 / 速度 / 音高）— 參考 §9
7. **Speech tab 繁中/簡中 voice 合併**：繁中的 voice dropdown 和 簡中的 voice dropdown **都顯示兩者 voice（zh-TW 全部 + zh-CN 全部）**，因為彼此可互用（繁中能念簡中文字、反之亦然）— 參考 §11

## 鎖死（MUST）

### 1. `src-tauri/src/window_state.rs`：擴 ClipboardMode

新 enum（檔首 or 合適位置）：
```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum ClipboardMode {
    None,
    OriginalOnly,
    TranslatedOnly,
    Both,
}

impl ClipboardMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ClipboardMode::None => "None",
            ClipboardMode::OriginalOnly => "OriginalOnly",
            ClipboardMode::TranslatedOnly => "TranslatedOnly",
            ClipboardMode::Both => "Both",
        }
    }
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "None" => Some(Self::None),
            "OriginalOnly" => Some(Self::OriginalOnly),
            "TranslatedOnly" => Some(Self::TranslatedOnly),
            "Both" => Some(Self::Both),
            _ => None,
        }
    }
}
```

WindowState 加欄位：
```rust
#[serde(default = "default_clipboard_mode")]
pub clipboard_mode: ClipboardMode,
fn default_clipboard_mode() -> ClipboardMode { ClipboardMode::OriginalOnly }
```

**向後相容 sanitize**（在 load_or_default 或類似的 load 流程中）：若舊檔沒 `clipboard_mode` 但有 `save_to_clipboard` + `translate_append_to_clipboard`：
- `save_to_clipboard=false` → `None`
- `save_to_clipboard=true, translate_append_to_clipboard=false` → `OriginalOnly`
- `save_to_clipboard=true, translate_append_to_clipboard=true` → `Both`

實作做法：在 load 後判斷 `if state.clipboard_mode == default 且 !state.save_to_clipboard` → 降級 None（這路徑比較 tricky，Codex 用最簡單可靠做法）。

**保留** `save_to_clipboard` + `translate_append_to_clipboard` 舊欄位不要刪（可能有其他 caller），但 write_capture 不再用它們。加 `#[deprecated]` 註解也行。

新 setter：
```rust
pub fn set_clipboard_mode(v: ClipboardMode) { update(|s| s.clipboard_mode = v); }
```

### 2. `src-tauri/src/clipboard.rs`：改用 mode

```rust
pub fn write_capture(original: &str, translated: &str) {
    let state = window_state::get();
    let text = match state.clipboard_mode {
        ClipboardMode::None => return,
        ClipboardMode::OriginalOnly => original.to_string(),
        ClipboardMode::TranslatedOnly => translated.to_string(),
        ClipboardMode::Both => {
            let sep = separator_char(&state.translate_separator);
            format!("{original}{sep}{translated}")
        }
    };
    if text.is_empty() { return; }
    // 其餘寫入邏輯不動
}
```

### 3. `src-tauri/src/commands/result_window.rs`：新 command

```rust
#[tauri::command]
pub fn set_clipboard_mode(value: String) -> Result<(), String> {
    match window_state::ClipboardMode::from_str(&value) {
        Some(mode) => { window_state::set_clipboard_mode(mode); Ok(()) }
        None => Err("invalid clipboard mode".into()),
    }
}
```

### 4. `src-tauri/src/lib.rs`：註冊 `set_clipboard_mode`

移除 `set_save_to_clipboard` / `set_translate_append_to_clipboard` 的註冊**改成保留**（為了 back-compat，不要砍舊 command）。

### 5. `src-tauri/src/tray.rs`：

**a. 儲存到剪貼簿 checkbox → submenu 4 radio**

```
儲存到剪貼簿 ▸
  ⦾ 不複製
  ⦾ 只複製原文
  ⦿ 只複製譯文
  ⦾ 複製原文+譯文
```

```rust
let clip_none = CheckMenuItem::with_id(app, "clip_none", "不複製", true, mode == ClipboardMode::None, None::<&str>)?;
let clip_original = CheckMenuItem::with_id(app, "clip_original", "只複製原文", true, mode == ClipboardMode::OriginalOnly, None::<&str>)?;
let clip_translated = CheckMenuItem::with_id(app, "clip_translated", "只複製譯文", true, mode == ClipboardMode::TranslatedOnly, None::<&str>)?;
let clip_both = CheckMenuItem::with_id(app, "clip_both", "複製原文+譯文", true, mode == ClipboardMode::Both, None::<&str>)?;
let clip_submenu = Submenu::with_items(app, "儲存到剪貼簿", true, &[&clip_none, &clip_original, &clip_translated, &clip_both])?;
```

互斥 handler（點 clip_* 時 4 個 set_checked）同 output lang pattern。

**b. 加「情境」submenu**（位置：輸出語言 submenu 下方，加 separator）

```
情境 ▸
  ⦿ 一般對話
  ⦾ 程式碼翻譯
  ⦾ （其他 scenarios）
```

install 時 snapshot：
```rust
let scenarios = crate::scenarios::list();
let active_id = crate::scenarios::active_id();
let scenario_items: Vec<CheckMenuItem> = scenarios.iter().map(|s|
    CheckMenuItem::with_id(app, format!("scenario_{}", s.id), s.name.clone(), true, s.id == active_id, None::<&str>).unwrap()
).collect();
let scenario_refs: Vec<&dyn IsMenuItem<_>> = scenario_items.iter().map(|x| x as &dyn IsMenuItem<_>).collect();
let scenario_submenu = Submenu::with_items(app, "情境", true, &scenario_refs)?;
```

（Codex 依 Tauri 2 menu API 實際可行的寫法調整；重點是能 render scenarios 動態清單）

Menu event 處理：
```rust
id if id.starts_with("scenario_") => {
    let sid = id.trim_start_matches("scenario_").to_string();
    let _ = crate::scenarios::set_active(&sid);
    // 更新 checked
    for item in &scenario_items {
        let _ = item.set_checked(item.id().as_ref() == format!("scenario_{sid}"));
    }
}
```

**c. listen scenarios-changed event** 更新 checked 狀態：

```rust
app.listen("scenarios-changed", move |_event| {
    let active = crate::scenarios::active_id();
    for item in &scenario_items_for_listener {
        let match_id = item.id().as_ref() == format!("scenario_{active}");
        let _ = item.set_checked(match_id);
    }
});
```

（**注意**：若使用者在 Settings 新增 scenario，tray submenu 不會出現新項目 — 要重啟 app 才看到。這是接受的限制。可在 scenario submenu label 或 Translate tab 補「新增/刪除情境後需重啟才反映到系統列」一行 hint 提示。）

**d. listen window-state-changed** 現有的 tray listener 要同步更新 clip_submenu 4 個 check：

```rust
app.listen("window-state-changed", move |event| {
    if let Ok(state) = serde_json::from_str::<WindowState>(event.payload()) {
        let _ = show_popup_clone.set_checked(state.popup_show_enabled);
        // 移除舊 save_clip_clone checkbox 更新
        // 新增 4 個 clip mode radio 互斥更新：
        let _ = clip_none_clone.set_checked(state.clipboard_mode == ClipboardMode::None);
        let _ = clip_original_clone.set_checked(state.clipboard_mode == ClipboardMode::OriginalOnly);
        let _ = clip_translated_clone.set_checked(state.clipboard_mode == ClipboardMode::TranslatedOnly);
        let _ = clip_both_clone.set_checked(state.clipboard_mode == ClipboardMode::Both);
    }
});
```

### 6. `src-tauri/src/scenarios.rs` 或相關 set_active 路徑：emit `scenarios-changed`

找 `scenarios::set_active` 或類似呼叫點（Codex 搜）：在 persist 成功後：
```rust
if let Some(app) = crate::app_handle::get() {
    use tauri::Emitter;
    let _ = app.emit("scenarios-changed", ());
}
```

（payload 用 `()` 或 active id string 都行，tray listener 只需要 trigger 就會去讀 current active）

**也要在** scenarios 新增 / 刪除 command 路徑 emit（雖然 tray 不會新增項目，但 listener 跑 set_checked 無害）。

### 7. `src/settings/tabs/TranslateTab.tsx`：**移除**

- 「☐ 將翻譯文附加到剪貼簿」checkbox
- 「分隔符號」dropdown
- 對應的 state / handler（`appendTranslation`, `separator`, `toggleAppendTranslation`, `changeSeparator`, normalizeSeparator）
- refresh() 裡面讀 `ws.translate_append_to_clipboard` / `ws.translate_separator` 可移除

保留：
- Output Language radio 5 語
- 翻譯情境 section（scenarios CRUD）

### 8. `src/settings/tabs/OutputTab.tsx`：**重寫**

「儲存到剪貼簿」section 改成：

```tsx
type ClipboardMode = "None" | "OriginalOnly" | "TranslatedOnly" | "Both";
type Separator = "Space" | "Tab" | "LineBreak" | "Comma" | "Semicolon" | "Pipe";

const CLIPBOARD_MODES: { code: ClipboardMode; label: string }[] = [
  { code: "None", label: "不複製" },
  { code: "OriginalOnly", label: "只複製原文" },
  { code: "TranslatedOnly", label: "只複製譯文" },
  { code: "Both", label: "複製原文+譯文" },
];

// state:
const [clipMode, setClipMode] = useState<ClipboardMode>("OriginalOnly");
const [separator, setSeparator] = useState<Separator>("Space");

// JSX:
<section className="settings-section">
  <h2>儲存到剪貼簿</h2>
  <div className="settings-radio-col">
    {CLIPBOARD_MODES.map(opt => (
      <label key={opt.code}>
        <input type="radio" name="clip-mode"
          checked={clipMode === opt.code}
          onChange={() => updateClipMode(opt.code)} />
        {opt.label}
      </label>
    ))}
  </div>
  {clipMode === "Both" && (
    <div style={{ marginTop: 10, paddingLeft: 24 }}>
      <label>
        分隔符號
        <select value={separator} onChange={e => updateSeparator(e.target.value as Separator)}>
          <option value="Space">空格</option>
          <option value="Tab">Tab</option>
          <option value="LineBreak">換行</option>
          <option value="Comma">逗號</option>
          <option value="Semicolon">分號</option>
          <option value="Pipe">豎線</option>
        </select>
      </label>
    </div>
  )}
</section>
```

handler：
```tsx
async function updateClipMode(next: ClipboardMode) {
  setClipMode(next);
  try { await invoke("set_clipboard_mode", { value: next }); }
  catch (err) { setStatusMsg(String(err)); }
}
async function updateSeparator(next: Separator) {
  setSeparator(next);
  try { await invoke("set_translate_separator", { value: next }); }
  catch (err) { setStatusMsg(String(err)); }
}
```

refresh 裡加：
```tsx
setClipMode((ws.clipboard_mode ?? "OriginalOnly") as ClipboardMode);
setSeparator((ws.translate_separator ?? "Space") as Separator);
```

window-state-changed listener 也更新 clipMode / separator。

### 9. `src/settings/SettingsView.css`：加 + 改 slider 垂直排列

加：
```css
.settings-radio-col {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.settings-radio-col label {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
}
```

**改既有 `.settings-slider-row`**（目前是 3 欄 grid，slider 水平並排擠壓 + text label 跟 slider 重疊）：
```css
.settings-slider-row {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.settings-slider-row label {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 12px;
}
.settings-slider-row input[type="range"] {
  width: 100%;
}
```

效果：音量/速度/音高 3 個 slider 垂直堆疊，每個 label 在自己的 slider 上方，互不擠壓。

### 10. 保留「顯示彈窗」checkbox + 「記錄到檔案」section 不動

### 11. `src/settings/tabs/SpeechTab.tsx`：繁中/簡中 voice 合併

目前 `voicesByLang` 是依 `voice.lang` 分組。改成：對 zh-TW / zh-CN 兩個 key 都 return 「zh-TW 全部 + zh-CN 全部」。

```tsx
const voicesByLang = useMemo(() => {
  const map: Record<string, TtsVoice[]> = {};
  voices.forEach(v => { (map[v.lang] ??= []).push(v); });

  // 合併繁中與簡中 — 互可用
  const zhCombined = [...(map["zh-TW"] ?? []), ...(map["zh-CN"] ?? [])];
  if (zhCombined.length > 0) {
    map["zh-TW"] = zhCombined;
    map["zh-CN"] = [...zhCombined];  // 獨立 array，避免 mutate
  }
  return map;
}, [voices]);
```

**顯示名要能分辨來源**：現在 display_name 像 `HsiaoChen (女)`，合併後看不出是 zh-TW 還是 zh-CN 的 voice。為了視覺區分，在合併時標 locale 前綴：

```tsx
const zhCombined = [
  ...(map["zh-TW"] ?? []).map(v => ({ ...v, display_name: `[繁] ${v.display_name}` })),
  ...(map["zh-CN"] ?? []).map(v => ({ ...v, display_name: `[簡] ${v.display_name}` })),
];
```

這樣 dropdown 看得出來哪些是 zh-TW voice、哪些是 zh-CN voice，但都可選。

**選中時 lang 字段用當下 panel 的 lang**（updateVoice(lang, code) 已傳 lang）— 即便 user 在繁中 panel 選了簡中 voice，storage 仍寫 speech_voices["zh-TW"] = "zh-CN-XiaoxiaoNeural"。這是刻意的（user 的偏好），後續使用時直接取這個 code 餵 TTS 引擎（Edge TTS 用 voice code 本身決定發音，跟 locale key 無關）。

其他 3 個 locale（en-US / ja-JP / ko-KR）不變。

## 禁動

- **不刪** `save_to_clipboard` / `translate_append_to_clipboard` 欄位（只加新 `clipboard_mode`），向後相容
- **不動** 既有 set_save_to_clipboard / set_translate_append_to_clipboard command（留著）
- **不動** VLM / TTS / Hotkey 流程
- **不改** capture/log / Ollama health 邏輯

## 驗證

- `cargo check` + `cargo build` + `npm build` 全過
- 編碼 UTF-8 NoBOM

## 風險點

1. **serde default 跟向後相容**：舊 window_state.json 沒 clipboard_mode 欄位 → `#[serde(default)]` 回 `OriginalOnly`，同時 load 後 sanitize 根據舊 save/append 欄位 override（若舊檔有 `save_to_clipboard=false`）。Codex 判斷最簡寫法。
2. **tray scenarios submenu API**：Tauri 2 的 `Submenu::with_items` 收 `&[&dyn IsMenuItem]`，動態 vec 寫法要注意 lifetime。若複雜，Codex 先建 Vec<CheckMenuItem> owned，再建立 references Vec，然後 pass slice。
3. **scenarios-changed listener 在 install 內註冊**：clone Vec<CheckMenuItem> 到 closure 可能需要 `Arc` 或每個 item 各自 clone。Codex 依 tauri 2 的 CheckMenuItem clone cost 判斷。

## 回報

```
=== T43 套改結果 ===
- window_state.rs 加 ClipboardMode enum + clipboard_mode 欄位 + setter + 向後相容
- clipboard.rs write_capture 改用 clipboard_mode
- commands/result_window.rs 新 set_clipboard_mode
- lib.rs 註冊
- tray.rs 剪貼簿 submenu 4 radio + 情境 submenu + listen scenarios-changed
- scenarios 寫入路徑 emit scenarios-changed
- TranslateTab.tsx 移除 append + separator
- OutputTab.tsx 加 radio 4 + separator (conditional)
- SettingsView.css 加 settings-radio-col
- cargo check: <結果>
- npm build: <結果>
- UTF-8 NoBOM

VERDICT: APPROVED
```

**直接套改**。不需要先給 diff 提案（規格詳盡）。UTF-8 NoBOM。
