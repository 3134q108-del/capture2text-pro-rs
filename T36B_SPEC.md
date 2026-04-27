# T36b · TranslateTab 加 Output Language 5 語 radio + Append + Separator

## 背景

T36a 已擴 output_lang 為 5 語（繁/簡/英/日/韓）。
T28b 已繁中化 TranslateTab 的 Scenarios UI。
現在要在 TranslateTab 頂部加三塊新區：
1. **輸出語言** radio × 5
2. **☐ 翻譯文附加到剪貼簿** checkbox（連動 window_state.translate_append_to_clipboard）
3. **分隔符號** dropdown 6 選項（連動 window_state.translate_separator）

下方保持現有的 Scenarios 編輯 UI。

## 目標

擴 `src/settings/tabs/TranslateTab.tsx`，只動這一檔（加上 SettingsView.css 為新 class 補樣式）。

## 鎖死（MUST）

### 新增 state

```tsx
type OutputLang = "zh-TW" | "zh-CN" | "en-US" | "ja-JP" | "ko-KR";
type Separator = "Space" | "Tab" | "LineBreak" | "Comma" | "Semicolon" | "Pipe";
type WindowState = {
  translate_append_to_clipboard: boolean;
  translate_separator: string;
  // ...忽略其他欄位
};

const [outputLang, setOutputLang] = useState<OutputLang>("zh-TW");
const [appendTranslation, setAppendTranslation] = useState<boolean>(false);
const [separator, setSeparator] = useState<Separator>("Space");
```

### refresh() 擴（現有 fn，加新 invoke）

```tsx
async function refresh() {
  try {
    const [list, active, lang, ws] = await Promise.all([
      invoke<Scenario[]>("list_scenarios"),
      invoke<string>("get_active_scenario"),
      invoke<string>("get_output_language"),
      invoke<WindowState>("get_window_state"),
    ]);
    setScenarios(list);
    setActiveId(active);
    setOutputLang(normalizeLang(lang));
    setAppendTranslation(ws.translate_append_to_clipboard);
    setSeparator(normalizeSeparator(ws.translate_separator));
    // ... rest 同現有
  }
}

function normalizeLang(s: string): OutputLang {
  return (["zh-TW","zh-CN","en-US","ja-JP","ko-KR"] as const).includes(s as OutputLang)
    ? (s as OutputLang) : "zh-TW";
}
function normalizeSeparator(s: string): Separator {
  return (["Space","Tab","LineBreak","Comma","Semicolon","Pipe"] as const).includes(s as Separator)
    ? (s as Separator) : "Space";
}
```

### 新增 3 個 handler

```tsx
async function changeOutputLang(next: OutputLang) {
  try {
    await invoke("set_output_language", { lang: next });
    setOutputLang(next);
    setStatusMsg("輸出語言已更新。");
  } catch (err) { setStatusMsg(String(err)); }
}

async function toggleAppendTranslation(checked: boolean) {
  try {
    await invoke("set_translate_append_to_clipboard", { value: checked });
    setAppendTranslation(checked);
    setStatusMsg("已更新翻譯附加設定。");
  } catch (err) { setStatusMsg(String(err)); }
}

async function changeSeparator(next: Separator) {
  try {
    await invoke("set_translate_separator", { value: next });
    setSeparator(next);
    setStatusMsg("分隔符號已更新。");
  } catch (err) { setStatusMsg(String(err)); }
}
```

### JSX 加在 return 最上方（在既有 `<div className="settings-tab-layout">` 外層包一個 wrapper；或改 return 結構）

**建議結構**（整個 return 加一層 `.settings-translate-root` wrapper）：
```tsx
return (
  <div className="settings-translate-root">
    <section className="settings-section">
      <h2>輸出語言</h2>
      <div className="settings-radio-row">
        {LANG_OPTIONS.map(opt => (
          <label key={opt.code}>
            <input type="radio" name="output-lang"
              checked={outputLang === opt.code}
              onChange={() => changeOutputLang(opt.code)} />
            {opt.label}
          </label>
        ))}
      </div>
    </section>

    <section className="settings-section">
      <label className="settings-checkbox">
        <input type="checkbox"
          checked={appendTranslation}
          onChange={(e) => toggleAppendTranslation(e.target.checked)} />
        將翻譯文附加到剪貼簿（原文後接分隔符再接譯文）
      </label>
    </section>

    <section className="settings-section">
      <label>
        分隔符號
        <select value={separator} onChange={(e) => changeSeparator(e.target.value as Separator)}>
          <option value="Space">空格</option>
          <option value="Tab">Tab</option>
          <option value="LineBreak">換行</option>
          <option value="Comma">逗號</option>
          <option value="Semicolon">分號</option>
          <option value="Pipe">豎線</option>
        </select>
      </label>
    </section>

    <section className="settings-section">
      <h2>翻譯情境</h2>
      <div className="settings-tab-layout">
        {/* 既有 Scenarios sidebar + editor 放這裡，JSX 原樣搬 */}
      </div>
    </section>
  </div>
);
```

### LANG_OPTIONS 常數（放檔案頂）

```tsx
const LANG_OPTIONS: { code: OutputLang; label: string }[] = [
  { code: "zh-TW", label: "繁體中文" },
  { code: "zh-CN", label: "簡體中文" },
  { code: "en-US", label: "英文" },
  { code: "ja-JP", label: "日文" },
  { code: "ko-KR", label: "韓文" },
];
```

### CSS 加到 `src/settings/SettingsView.css`

```css
.settings-translate-root {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.settings-section {
  border: 1px solid var(--c2t-border);
  border-radius: var(--c2t-radius);
  background: var(--c2t-bg);
  padding: 10px 12px;
}

.settings-section > h2 {
  margin: 0 0 8px;
  font-size: 13px;
  font-weight: 600;
  color: var(--c2t-text);
}

.settings-radio-row {
  display: flex;
  gap: 14px;
  flex-wrap: wrap;
}

.settings-radio-row label {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  cursor: pointer;
}

.settings-checkbox {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
}

/* Scenarios 區內部的 settings-tab-layout 保留原 grid 佈局 */
.settings-section > .settings-tab-layout {
  margin: 0;
  height: 280px; /* 給情境編輯區一個固定高度 */
}
```

## 禁動

- **不動** Scenarios 相關 state / handler / JSX 內部結構（完整搬，只包進 section）
- **不動** healthWarning（在父）
- **不動** 其他 tab 檔
- **不動** Rust 任何檔（T28 + T36a 已暴露所有需要的 command）

## 驗證

- `npm.cmd run build`（repo root）通過
- 手測略過（等全部做完一次測）

## 回報

```
=== T36b 套改結果 ===
- TranslateTab.tsx 擴加 3 塊：Output Language radio 5 語 + Append checkbox + Separator dropdown
- SettingsView.css 加新 class
- UTF-8 NoBOM
- npm build: <結果>

VERDICT: APPROVED
```

**直接套改，不需要先給 diff 提案**。全部 UTF-8 NoBOM。
