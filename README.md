# Capture2Text Pro

Windows 桌面 OCR + 翻譯。按 Win+Q 框選螢幕,本地 VLM 辨識,可選 Azure TTS 朗讀。

從 Christopher Brochtrup 的 [C++ 版 Capture2Text](https://capture2text.sourceforge.net/) 衍生(原版已停止維護),用 Tauri 2 + Rust 重寫。OCR 從 Tesseract 換成本地 VLM,加上翻譯與雲端 TTS。

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

## 系統需求 / 硬體性能 / 耗能

**為了在 7 語上做到不錯的 OCR + 翻譯品質,選用了 8B 級 VLM(Qwen3-VL-8B-Instruct Q4_K_M),而不是更輕量的專用 OCR 模型(像 Tesseract / PaddleOCR)。這是為了高品質的體驗刻意選的取捨,代價是吃硬體比較兇,先說清楚:**

基本門檻:

- **OS**:Windows 10 / 11(x64)
- **RAM**:16 GB 起跳(model 常駐約 5–6 GB,加上系統 + browser)
- **硬碟**:約 6 GB(模型)
- **GPU**:NVIDIA / AMD / Intel 隨意,純 CPU 也能跑,llama.cpp 自動選

待機 vs OCR + 翻譯中的實測:

| 狀態 | RAM / VRAM | GPU / CPU 負載 | 功耗(估) |
|---|---|---|---|
| **待機**(model 已載入,等熱鍵) | GPU 模式 ~5–6 GB VRAM;CPU 模式 ~5 GB RAM | idle | 接近 0 W |
| **OCR + 翻譯中**(每按 Win+Q,持續 1–10 秒) | 同上 | 滿載 100% | GPU:RTX 3060 ~170 W、RTX 4070 ~200 W、RTX 4090 ~450 W;CPU 滿載 ~45–80 W |

推論單次時間:**GPU 約 1–3 秒**,**純 CPU 約 5–10 秒**。

llama-server 在程式啟動時就把 model 載入並常駐,所以熱鍵響應只等推論時間,代價是 5–6 GB 記憶體一直佔住。在意省電 / 省記憶體的人不適合這支程式 — 拿低階筆電想跑這個會很吃力,建議至少 16 GB RAM + 中階獨顯。

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
