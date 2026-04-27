# T28 · Settings 4-tab shell

## 專案背景（fresh session，請先讀 STAGE7_PLAN.md 熟悉整體）

Capture2Text Tauri 2 rewrite。Stack：Rust（Tauri）+ React/TypeScript。
Win+Q/W/E 截圖 → 本機 Qwen3-VL 一條龍 OCR+翻譯 → popup + TTS。

Stage 7a 已完成 Popup 視窗（ResultView），commit 7bcc70d 是 Stage 7a 最新。
Stage 7b 開始：重做 Settings 視窗成 4-tab 結構，對齊原版 Capture2Text Qt5 UI（user 已逐頁拍板）。

**跟我 CC 的合作模式**：
- 你寫 100% 業務 code。我規劃、審查、驗證、commit。
- 送 diff 提案 → 等我 review → 套改 → 跑 `cargo check` + `npm.cmd run build` 驗證
- 回報格式：改哪些檔、每檔改什麼（diff 摘要），結尾標 `VERDICT: APPROVED` 或 `VERDICT: REVISE:<原因>`
- **不要自己跑 git add/commit**（.git 目錄權限是 MSYS2 uid 專屬，你會 permission denied）
- 遇到設計岔路先問我，不要自己決定

## 目標

把 `src/settings/SettingsView.tsx` 從現在的「單頁 Scenarios + TTS 混合」
改成「左 nav list + 右 stacked tab content + 底 OK/Cancel」的 4-tab shell。
同時在 Rust 端補齊後續 T36-T39 會用到的 commands skeleton。

**T28 只做 shell + stubs + Rust skeleton。4 個 tab 內容的細節由 T36-T39 各自填上。**
例外：為了不讓現有 Scenarios 和 TTS Voice 功能倒退，先把它們原封不動搬到對應 tab：
- 現有 Scenarios 編輯 UI → TranslateTab（T36 會再 enrich）
- 現有 TTS Voice 選單 → SpeechTab（T37 會再 enrich）

OutputTab、AboutTab 則純 stub（顯示「Coming soon」+ tab 標題即可）。

## 鎖死範圍（MUST）

### 接口 / 資料結構

**Rust 新增 command（全放 commands/result_window.rs）**：
```rust
#[tauri::command]
pub fn set_save_to_clipboard(value: bool) -> Result<(), String>

#[tauri::command]
pub fn set_popup_show_enabled(value: bool) -> Result<(), String>

#[tauri::command]
pub fn set_translate_append_to_clipboard(value: bool) -> Result<(), String>

#[tauri::command]
pub fn set_translate_separator(value: String) -> Result<(), String>
```
四個都只做：呼叫對應 window_state setter + 回 Ok(())。
空字串或非法值（Separator 不在 6 選項內）回 Err(String)。

**Rust 新檔 src-tauri/src/commands/output_lang.rs**：
```rust
#[tauri::command]
pub fn get_output_language() -> String  // 回傳當前 output_lang 值

#[tauri::command]
pub fn set_output_language(lang: String) -> Result<(), String>  // 寫入 + persist
```
呼叫現有的 `crate::output_lang::current()` 和 `crate::output_lang::set(&lang)`。

**window_state.rs 新增 setter（public fn）**：
```rust
pub fn set_save_to_clipboard(v: bool)
pub fn set_popup_show_enabled(v: bool)
pub fn set_translate_append_to_clipboard(v: bool)
pub fn set_translate_separator(v: String)  // 驗證在 command 層做，這裡直接存
```
模式跟現有的 `set_popup_topmost` 一樣，用既存的 `update` helper。

**lib.rs 更新**：
- 在模組宣告區加 `mod commands::output_lang;`（或在 commands/mod.rs 裡 `pub mod output_lang;`）
- `invoke_handler!` 裡註冊新的 6 個 commands

### React 新增檔案

```
src/settings/
  SettingsView.tsx         # 重寫成 shell
  SettingsView.css         # 淺色主題，對齊 src/styles/tokens.css
  tabs/
    TranslateTab.tsx       # 搬現有 Scenarios 編輯 UI
    SpeechTab.tsx          # 搬現有 TTS Voice 選單
    OutputTab.tsx          # stub：<p>Coming soon (T38)</p>
    AboutTab.tsx           # stub：<p>Coming soon (T39)</p>
```

### SettingsView.tsx 新 shell 結構

```tsx
// pseudo-structure
type TabKey = "translate" | "speech" | "output" | "about";

export default function SettingsView() {
  const [activeTab, setActiveTab] = useState<TabKey>("translate");
  const [healthWarning, setHealthWarning] = useState<HealthWarning | null>(null);

  // listen health-warning event（同現有邏輯）

  return (
    <div className="settings-root">
      {healthWarning && <div className="health-warning">⚠ {healthWarning.message}</div>}
      <div className="settings-layout">
        <nav className="settings-nav">
          <button className={activeTab === "translate" ? "active" : ""} onClick={() => setActiveTab("translate")}>Translate</button>
          <button className={activeTab === "speech" ? "active" : ""} onClick={() => setActiveTab("speech")}>Speech</button>
          <button className={activeTab === "output" ? "active" : ""} onClick={() => setActiveTab("output")}>Output</button>
          <button className={activeTab === "about" ? "active" : ""} onClick={() => setActiveTab("about")}>About</button>
        </nav>
        <main className="settings-content">
          {activeTab === "translate" && <TranslateTab />}
          {activeTab === "speech" && <SpeechTab />}
          {activeTab === "output" && <OutputTab />}
          {activeTab === "about" && <AboutTab />}
        </main>
      </div>
      <footer className="settings-footer">
        <button className="c2t-btn" onClick={handleCancel}>Cancel</button>
        <button className="c2t-btn c2t-btn-primary" onClick={handleOk}>OK</button>
      </footer>
    </div>
  );
}
```

- OK 按鈕：呼叫 `invoke("hide_settings_window")` 關窗（現階段 OK 就是關，不做 dirty-state，留給 T30）
- Cancel：現階段也呼叫 `hide_settings_window`（T30 才加 dirty state）
- 兩個按鈕現在行為相同 OK，但仍須存在以建立 UI 位置

### TranslateTab.tsx / SpeechTab.tsx 內容搬移

**TranslateTab.tsx**：把現有 SettingsView.tsx 的 Scenarios 編輯 UI（scenarios state、refresh、
selectScenario、createScenario、saveScenario、deleteScenario、applyActiveScenario、render 的
sidebar list + editor form + actions）整塊搬過來。listen "health-warning" event 留在
父 SettingsView 處理，TranslateTab 不管。Scenarios 相關的 invoke 呼叫（list_scenarios、
save_scenario 等）一模一樣保留。

**SpeechTab.tsx**：把現有 SettingsView.tsx 的 TTS Voice 區塊（voices state、ttsConfig state、
refresh、zhVoices/enVoices memo、setVoice、render 的 TTS Voice grid）搬過來。

兩個 tab 的 statusMsg 各自保留一份 local state（不在父共享）。

### 淺色主題（SettingsView.css 完全重寫）

```
使用 src/styles/tokens.css 的變數：
  --c2t-bg          背景
  --c2t-text        文字
  --c2t-text-muted  次要文字
  --c2t-border      一般邊框
  --c2t-border-focus focus 邊框
  --c2t-panel-bg    側邊 / footer 背景
  --c2t-btn-bg / --c2t-btn-hover / --c2t-btn-active
  --c2t-btn-primary-bg / --c2t-btn-primary-hover / --c2t-btn-primary-text
  --c2t-font-family / --c2t-font-size / --c2t-radius
```

layout 骨架：
- `.settings-root`：flex column、height:100vh、background: var(--c2t-bg)、color: var(--c2t-text)、font-family: var(--c2t-font-family)
- `.settings-layout`：grid-template-columns: 160px 1fr、flex:1、min-height:0
- `.settings-nav`：flex column、border-right: 1px solid var(--c2t-border)、background: var(--c2t-panel-bg)
- `.settings-nav button`：text-align:left、padding:10px 14px、background:transparent、border:none、color:var(--c2t-text)、hover:var(--c2t-btn-hover)、active:var(--c2t-btn-active) 或背景藍白反白
- `.settings-content`：padding:14px、overflow:auto
- `.settings-footer`：border-top:1px solid var(--c2t-border)、padding:10px 14px、display:flex、justify-content:flex-end、gap:8px、background: var(--c2t-panel-bg)
- `.c2t-btn`：通用按鈕類，邊框 var(--c2t-border)、bg var(--c2t-btn-bg)、hover/active 對應 token
- `.c2t-btn-primary`：bg var(--c2t-btn-primary-bg)、color var(--c2t-btn-primary-text)
- `.health-warning`：保留現有紅色警告（改成淺色主題對應：淡紅背景 #fff4f4、深紅邊框 #c83838、文字 #a32424）
- 既有 `.settings-list-item`、`.settings-editor` 等 class 保留在 SettingsView.css 但改成淺色配色（因為 TranslateTab 搬過去後還會用同樣 class name）

### 搬移的 class 命名不變

TranslateTab 和 SpeechTab 的 JSX 裡 className 保持跟原本一樣（.settings-list、.settings-editor、.settings-tts 等）。只改 CSS 顏色，不改結構。

## 禁動範圍（blacklist）

- **絕不動** `src/result/` 下任何檔（Popup 剛穩定）
- **絕不動** `src-tauri/src/tts/` 或 `src-tauri/src/vlm/`（Stage 7a 已驗）
- **絕不動** `src-tauri/src/tray.rs`（T40 會做）
- **絕不動** `src-tauri/src/output_lang.rs` 內的 sanitize 邏輯（目前只支援 zh/en 是故意的，T36 擴 5 語）
- **絕不動** `src/styles/tokens.css`
- **絕不改** tauri.conf.json（settings window size 不動）
- **絕不改** `src-tauri/capabilities/default.json`

## 驗收標準

**編譯**：
- `cargo check` in `src-tauri/` 通過
- 從 repo root：`npm.cmd run build` 通過（或至少 `npm.cmd run type-check` / tsc）

**功能手測**（我會跑）：
1. Win+Q 截圖，彈窗正常（不影響）
2. Tray 點 Settings → 看到新 4-tab shell，左側列出 Translate/Speech/Output/About
3. 預設 active = Translate，內容是現有 Scenarios + TTS Voice 編輯 UI（搬來的）

   **修正**：Scenarios 歸 Translate，TTS Voice 歸 Speech，兩者分開到各自 tab
4. 點 Speech → 看到 TTS Voice 選單（搬來的）
5. 點 Output → 看到「Coming soon (T38)」
6. 點 About → 看到「Coming soon (T39)」
7. 底部 OK / Cancel 各點一次都能關窗
8. 關窗後重新打開 Settings，狀態 reset 回預設 active tab = Translate
9. 淺色主題：背景白、文字黑、按鈕 Segoe UI 風格（對齊 Popup）

## 非目標（T28 不做）

- OK/Cancel dirty-state（T30 做）
- Output Language 5 語 radio（T36 做）
- Append + Separator（T36 做）
- Speech Volume/Rate/Pitch sliders（T37 做）
- Dynamic Edge TTS voice fetch（T37 做）
- Log to file 實作（T38 做）
- About 版本 / Ollama check / GitHub / 匯出匯入（T39 做）
- 刪除既有 `set_tts_voice` / `list_tts_voices` / `get_tts_config`（T37 重構）
- 改動 Scenarios 內部邏輯（只搬，不改）

## 風險點

1. **TranslateTab 和 SpeechTab 都依賴 refresh() 抓 list_scenarios + list_tts_voices 等**。
   搬移後各自 tab 的 useEffect 各自呼叫，會發生兩次 refresh。接受這個小重複（T36/T37 會重新組織）。

2. **healthWarning 留在父 SettingsView**。子 tab 不處理。

3. **既有 `hide_settings_window` command 已存在**，OK / Cancel 都呼它，無需新增。

4. **commands/mod.rs 要加 `pub mod output_lang;`**。別忘了。

5. **output_lang.rs 現在 sanitize 只認 zh/en**，set_output_language 任何非 en 的都會變 zh。這不是 bug，是 T36 會擴充。T28 只暴露 command skeleton，不動 sanitize。

## 回報格式

```
=== T28 DIFF 提案 ===

## Rust 改動
- window_state.rs: +N 行（4 個 setter）
- commands/result_window.rs: +N 行（4 個 command）
- commands/output_lang.rs: 新檔 +N 行
- commands/mod.rs: +1 行
- lib.rs: +6 行 invoke_handler 註冊

## React 改動
- src/settings/SettingsView.tsx: 重寫，約 N 行
- src/settings/tabs/TranslateTab.tsx: 新檔 N 行
- src/settings/tabs/SpeechTab.tsx: 新檔 N 行
- src/settings/tabs/OutputTab.tsx: 新檔 N 行
- src/settings/tabs/AboutTab.tsx: 新檔 N 行
- src/settings/SettingsView.css: 重寫 N 行

## 關鍵片段
[貼幾段 critical 的 code，讓我先 review 不要整份貼]

## 預計驗證指令
- cargo check
- npm.cmd run build

VERDICT: APPROVED  （或 REVISE:<一句話原因>）
```

**先給提案 diff，我 review 再套改。不要直接寫檔。**
