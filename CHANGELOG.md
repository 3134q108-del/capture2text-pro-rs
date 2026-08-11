# Changelog

## v0.6.0

### 模型系列更換：Qwen3.5（2B / 4B / 9B 三檔可選）
- 全系列從 Qwen3-VL 換成 **Qwen3.5**（Instruct、GGUF Q4_K_M），三個檔位可在「設定 → 模型」下載與切換：
  - **Qwen3.5-2B**（約 1.9 GB）：8 語，最快
  - **Qwen3.5-4B**（約 3.3 GB）：14 語，速度/品質甜蜜點
  - **Qwen3.5-9B**（約 6.3 GB）：20 語全支援，品質檔位
- 語言分級同步收斂為 **20 語 3 級**（主推 5 / 常用 7 / 進階 8）

### 延遲與穩定性（capture → 首字時間）
- **框選當下立即顯示結果視窗**（「辨識中」狀態），不再等首 token 才跳出
- 模型載入時顯示「模型啟動中」動畫；遇 503（載入中）自動重試而不是直接失敗
- **背景 watchdog**：llama-server crash 自動重啟（含 backoff），mid-stream crash 可恢復；每次 restart 後自動 warm up
- llama.cpp 升級至 **b9994**（staged 安裝，失敗自動回退）
- VRAM ≥ 8 GB 時 vision 部分預設 offload 到 GPU；flash-attn auto；縮小 batch buffer
- 內建 server 改用 **port 11500**，避開 Ollama 的 11434 衝突
- `perf.log` 記錄逐次 capture 的 TTFT breakdown（rotating）

### 翻譯品質
- **翻譯模式簡化**：移除 v0.4.4 的「智慧對翻（母語 ↔ 目標）」與母語設定，統一為「一律翻成目標語言」單一行為（原文已是目標語言時原文照出）
- 禁止第三語言混雜；strip `<text>` wrapper 洩漏
- 混合語言段落中嵌入的外語詞會一併翻譯
- temperature 降至 0.2 + 強化混語 fidelity；縮短翻譯 prompt 修正角括號內容不翻譯問題

### TTS
- **停止按鈕可中斷合成中的請求**；前端 timeout 隨文字長度縮放
- Region-safe 音色選擇 + fallback retry；speak 語言配對 atomic 化

### 擷取
- Win+W 掃描範圍加寬；擷取時隱藏游標、排除自家 overlay

## v0.5.0

### 三模式 uninstaller（installer 規範對齊）
NSIS uninstaller 提供：
- **模式選擇頁**：三 radio button（保守 / 部分 / 完全），預設保守
- **部分模式 checkbox 頁**：runtime 讀 `inventory.json` 動態列出可清項目，每項含當前大小
- **三模式對應清理**：minimal 只清 WebView/Cache 殘留；partial 依勾選清；full 全清 + cmdkey 刪 Azure TTS keyring entry
- **必要依賴受保護**：`bin/`（llama-server）在部分模式不顯示在 checkbox，只有「完全刪除」會清

### App inventory 維護
- 新增 `src-tauri/src/inventory.rs`：app 啟動時 + 下載模型 + 寫 OCR captures 時自動 reconcile `inventory.json`，供 uninstaller 讀取
- inventory 條目分類：ai-model / user-data / settings / cache / dependency；後者標 `removable: false`，部分模式不會顯示

### Bundle 改動
- **移除 MSI target**：WiX/MSI 無法掛 NSIS 三模式自訂頁面，bundle 只保留 `nsis`
- **NSIS template fork**：`src-tauri/windows/installer.nsi` fork 自 tauri-bundler 預設，加入兩個 `UninstPage custom`
- **nsJSON plugin 整合**：uninstaller 讀 inventory.json 用

### 內部變更（無 UX 影響）
- 移除舊 MessageBox-based「Delete app data」勾選（被三模式 wizard 取代）
- 多了 `%LOCALAPPDATA%\com.capture2text.pro\inventory.json` 檔（容量極小）

## v0.4.4

### 新功能：直接翻譯模式
- **2 選 1 翻譯模式**：智慧對翻（既有）/ 直接翻譯（新增）
  - 智慧對翻：抓母語 → 翻目標；抓其他 → 翻母語（雙向動態）
  - 直接翻譯：不論原文，永遠翻成目標語言（單向固定）
- **可從兩個地方切換**：設定 → 翻譯 tab radio / tray 系統選單「翻譯模式」submenu
- **雙向即時同步**：tray 切 → 設定即時跟進；設定切 → tray 即時跟進

### UI / UX 修正
- **「儲存語言設定」按鈕成功反饋**：按鈕文字依序「儲存中...」→「✅ 已儲存」(3 秒)，旁邊綠色「✅ 語言設定已儲存」訊息
- **移除「母語不能與目標語言相同」限制**：兩邊都允許設成同語言（在 Direct mode 下實用）
- **Tray「目標語言」submenu 雙向同步**：設定改目標 → tray 立即打勾在新值（v0.4.3 此路徑漏實作）

### 防呆 / 穩定性
- **修 tray 死鎖**：tray click handler `target_lang_*` / `toggle_show_popup` 不再持鎖呼叫 emit-trigger 函式，避免「開設定+點 tray」程式卡死
- **修 TranslateTab.tsx 編碼問題**：鎖死 NO BOM + LF only

## v0.4.3

文檔同步 patch（無功能變動）：

- **HelpTab**：語言數與使用流程說明更新為智慧雙向翻譯描述
- **tauri.conf.json**：installer `longDescription` 同步語言支援與「智慧雙向翻譯」字樣

## v0.4.2

### 翻譯核心
- **單次 pass 智慧雙向翻譯**：把翻譯方向決策塞進 system prompt，模型一次完成 OCR + 偵測 + 正確方向翻譯。修掉 v0.4.1 抓母語時「中文閃一下才切換成英文」的 UX glitch
- **JSON 解析 robustness**：加 llama.cpp `response_format=json_object` 強制 JSON 文法 + lenient fallback

### UI / 設定
- **TranslateTab 動態語言**：母語 / 目標下拉從 enabled_langs 讀取，儲存時不再覆蓋語言設定
- **LanguagesTab 友善標籤**：Tier 改為主推 / 常用 / 進階 + 副說明
- **SpeechTab 繁中化**；TranslateTab 補繁中
- **試聽即時停止**：播放期間按鈕變紅色「停止」，點下立即中斷
- **朗讀控制滑桿恢復**：朗讀速度（0.5x – 2.0x）+ 音量（-50% – +100%），同時套用至 Speak 與試聽
- **Azure 「儲存並測試」修復**：已 configured 時不輸入 key 也能單純「測試現有金鑰」

### Tray 系統選單
- **即時同步 enabled_langs**：改 LanguagesTab 後 tray「目標語言」立即更新，無需重啟

### 升級相容
- 舊版 `output_lang.txt` 自動合併進 `enabled_langs`
- 舊版 `azure_speech_rate` / `azure_speech_volume` 直接沿用
