# Capture2Text Pro

Windows 桌面 OCR + 翻譯。按 Win+Q 框選螢幕,本地 VLM 辨識,可選 Azure TTS 朗讀。

從 Christopher Brochtrup 的 [C++ 版 Capture2Text](https://capture2text.sourceforge.net/) 衍生(原版已停止維護),用 Tauri 2 + Rust 重寫。OCR 從 Tesseract 換成本地 VLM,加上翻譯與雲端 TTS。

## 為什麼做這個

學語言時,翻譯品質常常因為「沒上下文」卡關 — 同樣一句話在遊戲、技術文件、醫療場景應該翻得不一樣。這支程式針對**學習者**設計:

- **針對情境調翻譯**:看遊戲就用遊戲社群術語,讀程式碼就保留專有名詞,看醫療文件就走台灣醫學會慣用詞
- **聽 AI 自然語音念法**:Azure TTS 神經語音模擬母語者腔調,輔助發音 / 聽力學習,快速建立語感

## 功能

| 熱鍵 | 行為 |
|---|---|
| **Win+Q** | 框選螢幕區塊 → OCR + 翻譯 |
| **Win+W** | 游標往右抓一行 |
| **Win+E** | 游標雙向抓一行 |

- 7 語:繁中、簡中、英、日、韓、德、法
- 本地 VLM(離線):llama.cpp + Qwen3-VL-8B-Instruct(GGUF Q4_K_M,約 5 GB)
- Azure TTS BYOK(可選,F0 免費 tier 一般夠用)
- 結果視窗:複製、朗讀、編輯後再朗讀
- 同熱鍵連按會中斷正跑的 OCR,只跑最後一次
- 熱鍵自訂

## 情境(針對學習場景的翻譯模式)

每個情境是一段 prompt,告訴 VLM 「請用這種風格翻譯」。內建 5 個情境:

| 情境 | 用途 |
|---|---|
| **通用** | 預設,中性翻譯助理 |
| **航運 / 輪機** | 商船貨櫃船,保留 M/E、TEU、B/L、reefer、bunker 等英文專業縮寫並加中文註解 |
| **遊戲** | 遊戲社群慣用譯法,保留專有名詞(角色名 / 裝備 / 技能)|
| **程式碼 / 技術** | 保留 API 名 / 變數名 / 程式關鍵字,只譯註解和一般敘述 |
| **醫療** | 台灣醫學會慣用術語,不確定的英文加括號中文試譯 |

**也可以自訂情境**:設定 → 翻譯 tab → 「新增情境」,寫一段 prompt 告訴 VLM 你要的翻譯風格(例:「翻譯成 Z 世代年輕人用語」、「保留動漫專有名詞」、「商業合約風格,謹慎正式」)。每次 OCR 用「使用中」的情境跑。

## 系統需求 / 硬體性能 / 耗能

**為了在 7 語上做到不錯的 OCR + 翻譯品質,選用了 8B 級 VLM(Qwen3-VL-8B-Instruct Q4_K_M),而不是更輕量的專用 OCR 模型(像 Tesseract / PaddleOCR)。這是為了高品質的體驗刻意選的取捨,代價是吃硬體比較兇,先說清楚:**

基本門檻:

- **OS**:Windows 10 / 11(x64)
- **RAM**:16 GB 起跳(model 常駐約 5.4 GB,加上系統 + browser)
- **硬碟**:約 6 GB(模型)
- **GPU**:NVIDIA / AMD / Intel 都可,純 CPU 也能跑,llama.cpp 自動選

### 建議規格

| 等級 | CPU | RAM | GPU | OCR 推論時間 |
|---|---|---|---|---|
| **入門** | i5 / Ryzen 5 級別 | 16 GB | 內顯 / 純 CPU | 5–10 秒(慢但堪用) |
| **推薦** | i7 / Ryzen 7 級別 | 16–32 GB | RTX 3060 / 4060 (8+ GB VRAM) | 1–3 秒 |
| **發燒** | i9 / Ryzen 9 級別 | 32+ GB | RTX 4070 Ti+ | < 1.5 秒 |

### 實測數據(開發機)

```
CPU:  Intel Core i9-14900KF
GPU:  NVIDIA GeForce RTX 4070 Ti(12 GB VRAM)
RAM:  64 GB
```

| 狀態 | llama-server RAM | GPU VRAM | GPU 使用率 | CPU 使用率 |
|---|---|---|---|---|
| **待機**(model 載入,等熱鍵) | 5,403 MB | 7,377 MiB(含其他 GPU app baseline) | ~0%(程式自身) | < 1% |
| **OCR + 翻譯中**(1–3 秒) | 5,681 MB(peak) | 7,214 MiB(peak) | **70%(平均)/ 87%(peak)** | 5–10% |

推論單次時間在這台機器約 **1–3 秒**(包含 OCR + 翻譯)。CPU 模式(沒 GPU)推論會慢到 **5–10 秒**,且 CPU 會吃滿 70–100%(全 core)。

llama-server 在程式啟動時就把 model 載入並常駐,熱鍵響應只等推論時間,代價是 5.4 GB 記憶體一直佔住。在意省電 / 省記憶體的人不適合這支程式 — 拿低階筆電想跑這個會很吃力。

## 安裝

從 [Releases](../../releases) 抓 `Capture2Text Pro_*_x64-setup.exe` 雙擊裝。

## 使用

裝完啟動,右下角 tray icon 按右鍵 → 「設定」。

1. **輸出** — 選輸出語言、模型路徑(預設 `%LOCALAPPDATA%\com.capture2text.pro`)
2. **語音** — 貼 Azure Speech key + region(可跳過,只用 OCR + 翻譯)
3. **快捷鍵** — 改熱鍵(預設 Win+Q / W / E)

設好就 Win+Q 開始用。

## 開發

```bash
git clone https://github.com/3134q108-del/capture2text-pro-rs
cd capture2text-pro-rs
npm install
npm run tauri dev
```

需要 Rust(MSVC toolchain)、Node 18+、Visual Studio Build Tools。

打包:

```bash
npm run tauri build           # release
npm run tauri build -- --debug # debug(編譯較快,適合測 installer)
```

產出在 `src-tauri/target/{release,debug}/bundle/{nsis,msi}/`。

實作細節見 `docs/capture-spec.md`(Q/W/E 行為移植自 upstream C++ 版)。

## 已知限制

- 只在 Windows 測過,macOS / Linux 跑不起來(全域熱鍵和 tray 部分用 Win32 API)
- 純 CPU 推論一張約 5–10 秒,GPU 1–3 秒
- Azure TTS 要自備 key,F0 免費 tier 每月 50 萬字符
- Installer 沒簽名,第一次裝會看到 SmartScreen 警告,點「仍要執行」

## 致謝

- [Capture2Text](https://capture2text.sourceforge.net/) — Christopher Brochtrup 的原版,Q/W/E 互動延用此版本
- [llama.cpp](https://github.com/ggerganov/llama.cpp) — Georgi Gerganov 等
- [Qwen3-VL](https://huggingface.co/Qwen) — 阿里通義千問

## License

Apache License 2.0(`LICENSE`)。
