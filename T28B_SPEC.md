# T28b · Settings 繁體中文化

使用者指令：設定頁面全部繁體中文化。T28 已完成 shell，但還是英文，補 patch。

## 範圍（只改這 6 檔，**只改 string**，不動邏輯）

### SettingsView.tsx

- nav 4 個按鈕：
  - `Translate` → `翻譯`
  - `Speech` → `語音`
  - `Output` → `輸出`
  - `About` → `關於`
- footer 2 顆按鈕：
  - `OK` → `確定`
  - `Cancel` → `取消`

### TranslateTab.tsx

- 標籤：
  - `New Scenario` → `新增情境`
  - `Scenario ID` → `情境 ID`
  - `Scenario Name` → `情境名稱`
  - `Prompt` → `提示詞`
- 按鈕：
  - `Save` → `儲存`
  - `Set Active` → `設為使用中`
  - `Delete` → `刪除`
- Badge：
  - `Built-in` → `內建`
  - `Active` → `使用中`
- setStatusMsg 訊息：
  - `Created draft scenario.` → `已建立新情境草稿。`
  - `Scenario saved.` → `情境已儲存。`
  - `Scenario deleted.` → `情境已刪除。`
  - `Active scenario updated.` → `使用中情境已更新。`
  - `Scenario ID is required.` → `情境 ID 不能為空。`
  - `Scenario name is required.` → `情境名稱不能為空。`
- createScenario 預設名：`New Scenario` → `新情境`

### SpeechTab.tsx

- `TTS Voice` → `語音選擇`
- `Chinese (zh-TW)` → `中文（繁中）`
- `English (en-US)` → `英文`
- `TTS voice updated.` → `語音已更新。`

### OutputTab.tsx

- `Output` → `輸出`
- `Coming soon (T38)` → `開發中（T38）`

### AboutTab.tsx

- `About` → `關於`
- `Coming soon (T39)` → `開發中（T39）`

## 鎖死

- 只改 string，**不改 JSX 結構、state、invoke 參數、className**
- 全部寫入 UTF-8 NoBOM（用 `[System.IO.File]::WriteAllText(path, text, (New-Object System.Text.UTF8Encoding $false))`）
- 檔首尾空行保持原樣

## 驗證

- cargo check（src-tauri/）應過（本次不動 Rust，應無變化）
- npm.cmd run build（repo root）應過

## 非目標

- 不動 Rust 任何檔
- 不動 tray.rs（T40 會處理繁中）
- 不動 CSS
- 不加新功能

## 回報

```
=== T28b 套改結果 ===
- 6 個 .tsx 檔 UTF-8 NoBOM 已寫入
- cargo check: <結果>
- npm build: <結果>

VERDICT: APPROVED
```

**直接套改，不需要先給 diff 提案**（純 string replacement，風險極低）。
