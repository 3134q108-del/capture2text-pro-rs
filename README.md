# Capture2Text Pro

Windows 桌面 OCR + 智慧雙向翻譯 + Azure TTS 朗讀。按 Win+Q 框選螢幕，本地 VLM 辨識，自動雙向翻譯，可選 Azure TTS 朗讀。

從 Christopher Brochtrup 的 [C++ 版 Capture2Text](https://capture2text.sourceforge.net/) 衍生（原版已停止維護），用 Tauri 2 + Rust 重寫。OCR 從 Tesseract 換成本地 VLM（單次 pass 同時做 OCR + 智慧路由翻譯），加上雲端 TTS。

## 為什麼做這個

學語言時，翻譯品質常常因為「沒上下文」卡關 — 同樣一句話在遊戲、技術文件、醫療場景應該翻得不一樣。這支程式針對**學習者**設計：

- **智慧雙向翻譯**：抓母語內容 → 翻成目標語言（練習方向）；抓其他語言 → 翻成母語（看懂方向）。一個熱鍵涵蓋兩種學習場景。
- **針對情境調翻譯**：看遊戲就用遊戲社群術語、讀程式碼就保留專有名詞、看醫療文件就走台灣醫學會慣用詞。
- **聽 AI 自然語音念法**：Azure TTS 神經語音模擬母語者腔調，輔助發音 / 聽力學習，快速建立語感。

## 功能

| 熱鍵 | 行為 |
|---|---|
| **Win+Q** | 框選螢幕區塊 → OCR + 智慧翻譯 |
| **Win+W** | 游標往右抓一行 |
| **Win+E** | 游標雙向抓一行 |

- **智慧雙向翻譯（單次 pass）**：模型一次完成 OCR + 語言偵測 + 正確方向翻譯，不會「先翻錯方向再切回來」
- **32 語支援**（分四級品質）：使用者可在「設定 → 語言」自選啟用範圍
- **本地 VLM（離線）**：llama.cpp + Qwen3-VL-8B-Instruct（GGUF Q4_K_M，約 5 GB）
- **Azure TTS BYOK**（可選，F0 免費 tier 一般夠用）
  - 語速 / 音量 滑桿（套用至所有 Speak 與試聽）
  - 試聽支援即時停止（紅色停止按鈕）
- **結果視窗**：複製、朗讀、編輯後再朗讀
- **同熱鍵連按會中斷正跑的 OCR**，只跑最後一次
- **Tray 系統選單即時同步**：改設定後 tray 自動更新，不用重啟
- 熱鍵自訂

## 32 語與品質分級

支援的語言依 OCR + 翻譯 + TTS 的綜合品質分四級：

| 等級 | 數量 | 涵蓋語言 | 用途 |
|---|---|---|---|
| **主推語言** | 5 | zh-CN、zh-TW、en-US、ja-JP、ko-KR | OCR + 翻譯 + TTS 品質最佳，預設啟用 |
| **常用語言** | 7 | fr、de、es、pt、it、ru、vi | 歐美亞主流語系，品質良好 |
| **進階語言** | 8 | ar、id、th、hi、el、he、tr、pl | 含 RTL 或特殊字元，可運作但建議測試 |
| **實驗語言** | 12 | nl、uk、cs、sv、da、no、fi、hu、ro、bg、ms、fil-PH | 支援度有限，TTS 走英文 fallback 音色 |

「設定 → 語言」可勾選要啟用的語言；「設定 → 翻譯」選擇母語 + 目標語言。母語跟目標語言必須在啟用清單內。

## 智慧對翻邏輯

設定 `母語 = zh-TW`、`目標 = en-US` 時：

| 框到的內容 | 翻成 | 用途 |
|---|---|---|
| 中文（=母語） | 英文（=目標） | 練習目標語言 |
| 英文（=目標） | 中文（=母語） | 看懂內容 |
| 其他語言（西班牙文、德文等） | 中文（=母語） | 看懂內容（read mode） |

決策塞進 prompt，模型一次完成 OCR + 偵測 + 正確方向翻譯，UI streaming 從一開始就是正確語言，沒有過場閃爍。

## 情境（針對學習場景的翻譯模式）

每個情境是一段 prompt，告訴 VLM 「請用這種風格翻譯」。內建 5 個情境：

| 情境 | 用途 |
|---|---|
| **通用** | 預設，中性翻譯助理 |
| **航運 / 輪機** | 商船貨櫃船，保留 M/E、TEU、B/L、reefer、bunker 等英文專業縮寫並加中文註解 |
| **遊戲** | 遊戲社群慣用譯法，保留專有名詞（角色名 / 裝備 / 技能） |
| **程式碼 / 技術** | 保留 API 名 / 變數名 / 程式關鍵字，只譯註解和一般敘述 |
| **醫療** | 台灣醫學會慣用術語，不確定的英文加括號中文試譯 |

**也可以自訂情境**：設定 → 翻譯 tab → 「新增情境」，寫一段 prompt 告訴 VLM 你要的翻譯風格（例：「翻譯成 Z 世代年輕人用語」、「保留動漫專有名詞」、「商業合約風格，謹慎正式」）。每次 OCR 用「使用中」的情境跑。

## 系統需求 / 硬體性能 / 耗能

**為了在 32 語上做到不錯的 OCR + 翻譯品質，選用了 8B 級 VLM（Qwen3-VL-8B-Instruct Q4_K_M），而不是更輕量的專用 OCR 模型（像 Tesseract / PaddleOCR）。這是為了高品質的體驗刻意選的取捨，代價是吃硬體比較兇，先說清楚：**

基本門檻：

- **OS**：Windows 10 / 11（x64）
- **RAM**：16 GB 起跳（model 常駐約 5.4 GB，加上系統 + browser）
- **硬碟**：約 6 GB（模型）
- **GPU**：NVIDIA / AMD / Intel 都可，純 CPU 也能跑，llama.cpp 自動選

### 建議規格

| 等級 | CPU | RAM | GPU | OCR 推論時間 |
|---|---|---|---|---|
| **入門** | i5 / Ryzen 5 級別 | 16 GB | 內顯 / 純 CPU | 5–10 秒（慢但堪用） |
| **推薦** | i7 / Ryzen 7 級別 | 16–32 GB | RTX 3060 / 4060（8+ GB VRAM） | 1–3 秒 |
| **發燒** | i9 / Ryzen 9 級別 | 32+ GB | RTX 4070 Ti+ | < 1.5 秒 |

### 實測數據（開發機）

```
CPU:  Intel Core i9-14900KF
GPU:  NVIDIA GeForce RTX 4070 Ti（12 GB VRAM）
RAM:  64 GB
```

| 狀態 | llama-server RAM | GPU VRAM | GPU 使用率 | CPU 使用率 |
|---|---|---|---|---|
| **待機**（model 載入，等熱鍵） | 5,403 MB | 7,377 MiB（含其他 GPU app baseline） | ~0%（程式自身） | < 1% |
| **OCR + 翻譯中**（1–3 秒） | 5,681 MB（peak） | 7,214 MiB（peak） | **70%（平均）/ 87%（peak）** | 5–10% |

推論單次時間在這台機器約 **1–3 秒**（單次 pass：OCR + 翻譯一次完成）。CPU 模式（沒 GPU）推論會慢到 **5–10 秒**，且 CPU 會吃滿 70–100%（全 core）。

llama-server 在程式啟動時就把 model 載入並常駐，熱鍵響應只等推論時間，代價是 5.4 GB 記憶體一直佔住。在意省電 / 省記憶體的人不適合這支程式 — 拿低階筆電想跑這個會很吃力。

## 安裝

從 [Releases](../../releases) 抓 `Capture2Text Pro_*_x64-setup.exe` 雙擊裝。

> **若安裝時跳「從伺服器傳回一個轉介」（ERROR_REFERRAL_RETURNED）**：
> 你的機器有 UAC policy `ValidateAdminCodeSignatures = 1`（強迫所有提權的 EXE 必須有 Authenticode 簽章；通常是企業安全軟體留下的設定），未簽章的 installer 被擋。以系統管理員開 PowerShell 跑：
> ```powershell
> reg add "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System" /v ValidateAdminCodeSignatures /t REG_DWORD /d 0 /f
> ```
> 恢復 Microsoft 預設值即可（不會降低安全，Defender + UAC + SmartScreen 仍正常運作）。

## 使用

裝完啟動，右下角 tray icon 按右鍵 → 「設定」。

1. **語言** — 勾選要啟用的語言（預設啟用主推 5 語）
2. **翻譯** — 選母語 + 目標語言（智慧對翻會在這兩個方向之間自動切換）；可選擇情境（通用 / 航運 / 遊戲 / 程式碼 / 醫療）或自建
3. **語音** — 貼 Azure Speech key + region（可跳過，只用 OCR + 翻譯）；指定每個語言的音色；調整朗讀速度與音量
4. **快捷鍵** — 改熱鍵（預設 Win+Q / W / E）

設好就 Win+Q 開始用。

## 開發

```bash
git clone https://github.com/3134q108-del/capture2text-pro-rs
cd capture2text-pro-rs
npm install
npm run tauri dev
```

需要 Rust（MSVC toolchain）、Node 18+、Visual Studio Build Tools。

打包：

```bash
npm run tauri build           # release
npm run tauri build -- --debug # debug（編譯較快，適合測 installer）
```

產出在 `src-tauri/target/{release,debug}/bundle/{nsis,msi}/`。

實作細節見 `docs/capture-spec.md`（Q/W/E 行為移植自 upstream C++ 版）。

## 已知限制

- 只在 Windows 測過，macOS / Linux 跑不起來（全域熱鍵和 tray 部分用 Win32 API）
- 純 CPU 推論一張約 5–10 秒，GPU 1–3 秒
- Azure TTS 要自備 key，F0 免費 tier 每月 50 萬字符
- Installer 沒簽名，第一次裝會看到 SmartScreen 警告，點「仍要執行」；如機器有 `ValidateAdminCodeSignatures=1`（見「安裝」段）需先關閉

## 致謝

- [Capture2Text](https://capture2text.sourceforge.net/) — Christopher Brochtrup 的原版，Q/W/E 互動延用此版本
- [llama.cpp](https://github.com/ggerganov/llama.cpp) — Georgi Gerganov 等
- [Qwen3-VL](https://huggingface.co/Qwen) — 阿里通義千問

## License

Apache License 2.0（`LICENSE`）。

---

## v0.4.4 改動

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
- **修 TranslateTab.tsx 編碼問題**：v0.4.3 之前 Codex 改檔曾誤用 UTF-8 with BOM 導致中文字串亂碼，本版重做 + 鎖死 NO BOM + LF only

## v0.4.3 改動

文檔同步 patch（無功能變動）：

- **HelpTab**：「7 種語言全本機處理」→「32 種語言」；「使用流程 → 需要做的設定」改為母語 + 目標語言雙向描述（含智慧對翻方向說明），原本誤導為單向「翻譯目標語言」的單句說明
- **tauri.conf.json**：installer `longDescription` 從「7 語(繁中/簡中/英/日/韓/德/法)」更新為「32 語(主推 5 + 常用 7 + 進階 8 + 實驗 12)」+ 加「智慧雙向翻譯」字樣

## v0.4.2 重點改動

### 翻譯核心
- **單次 pass 智慧雙向翻譯**：把翻譯方向決策塞進 system prompt，模型一次完成 OCR + 偵測 + 正確方向翻譯。修掉 v0.4.1 抓母語時「中文閃一下才切換成英文」的 UX glitch
- **JSON 解析 robustness**：加 llama.cpp `response_format=json_object` 強制 JSON 文法 + lenient fallback，避免模型偶爾回 plain text 時直接報錯

### UI / 設定
- **TranslateTab 動態語言**：母語 / 目標下拉現在從 enabled_langs 讀取（含 fr/de 等使用者啟用語言），儲存時不再覆蓋語言設定
- **LanguagesTab 友善標籤**：Tier S/A/B/C → 主推 / 常用 / 進階 / 實驗 + 副說明
- **SpeechTab 繁中化**：完整翻譯（保留 Azure 品牌名與 voice ID）
- **TranslateTab 補繁中**：「內建」「使用中」「提示詞」
- **試聽即時停止**：播放期間按鈕變紅色「停止」，點下立即中斷
- **朗讀控制滑桿恢復**：朗讀速度（0.5x – 2.0x）+ 音量（-50% – +100%），同時套用至 Speak 與試聽
- **Azure 「儲存並測試」修復**：已 configured 時不輸入 key 也能單純「測試現有金鑰」

### Tray 系統選單
- **即時同步 enabled_langs**：改 LanguagesTab 後 tray「目標語言」立即更新，無需重啟

### 32 語架構
- 主推 5（zh-CN / zh-TW / en-US / ja-JP / ko-KR）
- 常用 7（fr / de / es / pt / it / ru / vi）
- 進階 8（ar / id / th / hi / el / he / tr / pl）
- 實驗 12（nl / uk / cs / sv / da / no / fi / hu / ro / bg / ms / fil-PH）

### 升級相容
- 舊版 `output_lang.txt` 自動合併進 `enabled_langs`
- 舊版 `azure_speech_rate` / `azure_speech_volume` 直接沿用（v0.4.0 大瘦身時 UI 滑桿被刪掉，本版恢復）
