# Capture2Text Pro 專案交接文件

**日期**：2026-04-25
**最後 commit**：`7bcc70d` (Stage 7a T35j) — **之後所有改動 (~3042 行 diff) 都尚未 commit**
**Repo**：`D:\興趣專案\Github\Capture2Text 專案\rust\capture2text-pro-rs`
**OS**：Windows 11，繁中環境
**Hardware**：RTX 4070 Ti 12 GB VRAM、12+ core CPU、Intel Alderlake

---

## 1. 專案總覽

**Capture2Text Pro**：Windows 桌面 OCR + 翻譯 + 朗讀工具的 Tauri 2 (Rust + React) 重寫版。

- 前端：React + Vite + TypeScript (`src/`)
- 後端：Rust + Tauri 2 (`src-tauri/`)
- VLM：本機 llama.cpp (b8870, CUDA 12.4) + Qwen3-VL-4B-Instruct（5 主語）+ Pixtral-12B-2409（德法）
- TTS：本機 qwen3-tts crate (TrevorS GitHub fork, candle + cuda)
- Hotkey：Win+Q (區域)、Win+W (視窗)、Win+E (全螢幕) WH_KEYBOARD_LL hook

---

## 2. 已完成的階段

### Stage 0–6：基礎建設（已 commit）
Hotkey、capture、tray、tesseract → Ollama VLM 切換、Edge TTS、scenarios、settings shell。

### Stage 7（已 commit 到 7bcc70d）
Settings 4-tab 重寫、Tray 改 Output Language submenu、Edge TTS 序列化解 RST、cacheReady gating、Pop-up 重寫含 design tokens、 settings/popup window state 持久化。

### T28-T48（uncommitted，已穩定運作）
- **T28-T42**：Settings 全繁中化 + 4 tab 內容（翻譯/語音/輸出/關於）+ Output language 5 語擴充 + Tray 重寫 + Clipboard pipeline + cross-window event sync。
- **T43**：Output tab 改 4 radio (不複製/原文/譯文/原文+譯文) + Tray 加情境 submenu + slider 垂直 + 繁簡中 voice 合併 + skipTaskbar fix。
- **T44**：VLM keep_alive + warmup + slider overflow CSS 修。
- **T45**：Ollama 自動啟動（已廢棄，T50a 替換成 llama.cpp）。
- **T46**：keep_alive 30m → 5m 解 GPU 常駐 lag。
- **T47**：略過（streaming partial 節流，後來不需要）。
- **T48**：UI spacing tokens 統整 + Ollama CLI serve。

### T50a-T50b（uncommitted，已 work）— **VLM 換 llama.cpp**
- **T50a**：完全廢棄 Ollama，改 llama.cpp。新模組 `llama_runtime/`（`mod` + `manifest` + `downloader` + `supervisor`）。app 啟動時自動下載：
  - `llama-server.exe` (b8870 cuda-12.4 zip)
  - `cudart-llama-bin-win-cuda-12.4-x64.zip`
  - Qwen3-VL-4B-Instruct GGUF + mmproj-F16
  - Pixtral-12B-2409 GGUF + mmproj-F16
  - 全部存 `%LOCALAPPDATA%\Capture2TextPro\models\`
  - llama-server spawn 用 ASCII path junction + vcvars64.bat
  - **`--n-gpu-layers 20`**（partial offload，省 VRAM 給 TTS）
  - **`--flash-attn on`**（b8870 後 flag 改 value 形式）
  - **size verify**：bootstrap 檢查每個 model 檔的 HEAD Content-Length 跟 local 對比，不對就重下
- **T50a hotswap**：原本 8B → **4B-Instruct**（VRAM 從 7.4 GB 降到 ~3 GB）
- **T50b**：output_lang 擴 7 語（zh-TW / zh-CN / en-US / ja-JP / ko-KR / **de-DE / fr-FR**）。Pixtral 自動切換邏輯：每次 VLM job 前 `ensure_model_for_lang(lang)`，de/fr 切到 Pixtral，其他用 Qwen3-VL-4B。Tray submenu + Settings Translate radio 同步 7 語。

### Ollama 完全移除
- 透過 PS uninstaller + 清 `.ollama/` (14 GB) + Registry startup + Start Menu。釋放 ~15 GB 磁碟。

### CUDA 12.4 已安裝
- 透過 `winget install Nvidia.CUDA --version 12.4 --interactive` 完成。
- 路徑 `C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v12.4\`。
- `nvcc 12.4.131` 在 PATH。
- `CUDA_PATH` env var 設好。

### Avira 排除規則（user 自己加好）
- `D:\興趣專案\Github\Capture2Text 專案\` 整個資料夾
- `C:\Users\Home\bin\`
- `C:\Users\Home\.cargo\`

---

## 3. 進行中：T51 Qwen3-TTS 整合（**核心痛點**）

**目標**：用本機 GPU TTS 取代 Edge TTS（雲端網路 RST + 5-15s 延遲）。

### 已完成
- **Cargo.toml**：`qwen3-tts = { git = "https://github.com/TrevorS/qwen3-tts-rs", features = ["hub", "cuda"] }`
- 新模組 `src-tauri/src/qwen_tts/`（`mod.rs` + `runtime.rs` + `downloader.rs`）
- `tts/mod.rs` 重寫成 facade 呼叫 qwen_tts
- `commands/tts.rs` 簡化（speak / list_voice_presets / set_active_preset）
- `vlm/mod.rs` 移除 prefetch（本地秒級合成不需 cache）
- `SpeechTab.tsx` 重寫：9 preset 列表 + 試聽 + 設為使用中
- `ResultView.tsx` 移除 cacheReady state + 「合成中」UI
- `window_state.speech_active_preset: String`（預設 "Ryan"）

### Crate 狀況
- TrevorS `qwen3-tts` GitHub-only crate (v0.3.0, candle + cuda)
- crates.io 版 `qwen3-tts-rs` v0.2.2 是另一作者用 libtorch（不要）
- crate checkout 在 `C:\Users\Home\.cargo\git\checkouts\qwen3-tts-rs-5d2d4045f53db325\711ceee\`
- crate 提供 `synthesize_with_voice` (non-streaming) + `synthesize_streaming` (StreamingSession iterator yielding `Result<AudioBuffer>`)

### Crate patch 紀錄（in-place 修改 cargo git checkout）
- `src/lib.rs:1438` `compute_dtype_for_device` 試過 BF16 → F16 → F32 → 又改回 **BF16**（最終）
- 因為 dtype 不是 root cause，root cause 在 SynthesisOptions 缺值

### Model 下載狀況
- `%LOCALAPPDATA%\Capture2TextPro\tts_models\customvoice\`（已下載完整）
- 三個 HF repo 拼起來：
  - `Qwen/Qwen3-TTS-12Hz-0.6B-CustomVoice` (主 model)
  - `Qwen/Qwen3-TTS-Tokenizer-12Hz` (audio tokenizer ~682 MB)
  - `Qwen/Qwen2-0.5B` (text tokenizer.json)

### 9 preset speakers 與 native language
| Speaker | Native lang | Speaker | Native lang |
|---------|-------------|---------|-------------|
| Serena | 中文 | Aiden | **英文** |
| Vivian | 中文 | OnoAnna | 日文 |
| UncleFu | 中文 | Sohee | 韓文 |
| **Ryan** | **英文** | Eric | 中文 |
| Dylan | 中文 | | |

來源：crate `models/talker.rs::Speaker::native_language()`

---

## 4. 當前阻塞點 & 診斷流程

### 問題：在 app 內按 Speak 失敗
歷時診斷（見 `T51A_*.txt` 系列指示檔在 `C:\Users\Home\`）：

1. **第一次 OOM (BF16 streaming)**：145s 後 `CUDA_ERROR_OUT_OF_MEMORY`
2. **改 `--n-gpu-layers 20` (T50a A) + 確認 BF16**：仍 OOM
3. **改 F16**：OOM
4. **改 F32**：合成成功但 chunks=205 (max_length 用完 = 164s 音訊但輸入 187 字應 60s) + max_abs=23（**幾乎靜音**）
5. **WAV header 顯示 sr=24000Hz ch=1 bits=16**，正常規格
6. **rodio 預設輸出 device 是 BenQ HDMI 螢幕**（user 已確認音訊輸出 OK，這不是問題）

### Root cause（**關鍵發現**）
我們用 `SynthesisOptions::default()` — 沒設 `temperature` / `top_k` / `max_length` → autoregressive 進入 random gibberish + 跑滿 max_length。

### 修法（已提供 spec 但 in-progress）
照 crate `examples/tts.rs` 用：
```rust
SynthesisOptions {
    temperature: 0.9,
    top_k: 30,
    max_length: 2048,
    chunk_frames: 10,
    ..Default::default()
}
```

### Isolated test 已驗證 ✅（純 crate API，bypass app）
- `src-tauri/src/bin/tts_isolated_test.rs` 直接呼叫 `model.synthesize_with_voice(text, Speaker::Ryan, Language::Chinese, options)` 存 WAV
- **WAV 路徑**：`D:\興趣專案\Github\Capture2Text 專案\rust\capture2text-pro-rs\src-tauri\isolated_test_ryan_zh.wav`
- **user 聽結果**：能正常聽 ✅（**crate + model + GPU 都正常**）
- **但機械感重 + 語速慢 + 不連貫**（因為 Ryan 是英文 native，硬念中文）

### 正在跑（換 session 時可能還沒完成）
擴展 isolated test：跑 4 個 voice (Ryan + Vivian + UncleFu + Dylan) × 中文文本，比較 RTF + 音色，spec 在 `C:\Users\Home\T51A_ZH_VOICES.txt`。

### 🚨 剛揭露的更深 root cause（2026-04-25 換 session 前）
Codex 跑 4-voice test 時發現：**單一 Rust process 內連續對 Qwen3TTS 多次 synthesize → 第二個 voice 開始 CUDA_OOM**（跟我們 app 反覆按 Speak 一樣）。

Codex 修法：把 isolated test 改成 **parent process spawn child process per voice**（CUDA context 完全 reset 才不爆）。

**對 app 的啟示**：
- 我們 app 是長期 single-process（按 Win+Q 多次、按 Speak 多次）
- candle-cuda 0.9 在 Windows + RTX 4070 Ti 環境下，**TTS forward pass 後 GPU memory 沒完全釋放**（candle drop tensor 不一定立即 free CUDA buffer）
- 所以 Speak 第一次可能 work，第 2-3 次必爆 OOM
- 這就是為何 user 反覆按 Speak 過程混亂（成功 / OOM / 雜訊交錯）

**真正修法選項**（給下個 session 評估）：
1. **每次 speak 開 subprocess** spawn `tts_isolated_test.exe --speaker X --text Y` → child process exit 時 CUDA context 全 release
   - 代價：每次 speak 多 ~3-5s 啟動 child + load model（model 已快取，主要是 candle init）
   - 但 `target/debug/tts_isolated_test.exe` 體積大且 dev-only，prod 要嵌入
2. **手動每次 synth 後 force GPU memory release**：
   - 看 candle 是否暴露 cudaMemPool reset / Device::cuda(0).synchronize() 等
   - drop model + recreate 每次（極慢）
3. **改 streaming + 強制每 chunk drop**：邊合成邊播 + chunk-level drop tensor（不等整段）— 可能讓 GPU 持續釋放
4. **TTS 跑 CPU**（最後 fallback，慢但無 OOM）

優先試 1（subprocess）or 3（streaming + drop）。

---

## 5. User 偏好決策

### 確認的決策
- **VLM**：Qwen3-VL-4B（5 語）+ Pixtral-12B（de/fr）兩 model 切換 — **已實作**
- **TTS**：Qwen3-TTS 0.6B CustomVoice — **已實作但 Speak in-app 還沒通**
- **下載策略**：App 內自動下載
- **Settings 語音 tab 架構**：4 section（基本設定 / Preset / 風格描述 / 克隆聲音）— **目前只做了 Preset list**
- **方案 A**：Preset + Voice Cloning（不做 VoiceDesign，不下 4 GB）

### 偏好的 voice
- HF Space demo 試聽偏好 **Aiden + Ryan**（**英文 native**）
- **問題**：他們念中文機械
- **解**：要嘛接受中文 native voice (Vivian/Dylan/UncleFu) + 認命音色，要嘛走 T51b voice cloning 嘗試 cross-language transfer (Ryan 音色念中文)

---

## 6. 下個 session 該做什麼

### 立刻先做
1. **接手 isolated 4-voice test 結果**（如果 Codex 還沒跑完則等完）
   - 4 個 WAV 在 `src-tauri/`：`isolated_test_ryan_zh.wav` / `vivian` / `unclefu` / `dylan`
   - 看每個 RTF 數字 + user 聽完哪個音色可接受
2. **依 user 決策定 default speaker**（在 `qwen_tts/mod.rs::VoicePreset` default 改）

### 接著做（fix in-app Speak）
3. **修 `qwen_tts/runtime.rs`** 加正確 SynthesisOptions：
   ```rust
   let opts = SynthesisOptions {
       temperature: 0.9,
       top_k: 30,
       max_length: 2048,
       chunk_frames: 10,
       ..Default::default()
   };
   ```
   spec 在 `C:\Users\Home\T51A_OPTIONS.txt`
4. cargo build (用 ASCII junction `C:\Users\Home\c2t-tauri` + vcvars64.bat 套路)
5. user restart dev 測 Win+Q + Speak

### 確認 work 後
6. **T51b voice cloning**（如果 user 要 Ryan 音色）：spec 在 `T51B_SPEC.md`，要下 Base model ~2 GB，新 CRUD UI（克隆聲音 list + 上傳音訊 + 命名 + 試聽 + 設為使用中），voices 存 `%LOCALAPPDATA%\Capture2TextPro\voices\cloned\`
7. **commit 全部改動**（從 7bcc70d 起 ~3042 行 diff，包含 Stage 7 後續到 T51）

---

## 7. 工作流規則（codex-collab）

**100% code 改動由 Codex 執行**。CC 寫 spec → cx-send → Codex 實作。

例外可直接寫：
- `~/.claude`、`~/.codex`、`~/bin`、`/tmp`、`/etc`、`/var`
- `*.md`、`*.txt`、`*.json`、`*.toml`、`*.yaml`、`*.yml`、`.env.example`、`.gitignore`

### Codex pane 操作
```bash
source ~/bin/cx-env
wezterm-cc cli list                          # 看 panes
wezterm-cc cli spawn --new-window --cwd "D:/興趣專案/Github/Capture2Text 專案/rust/capture2text-pro-rs" -- cmd
wezterm-cc cli send-text --pane-id <N> 'codex'
wezterm-cc cli send-text --pane-id <N> $'\r'  # Enter
wezterm-cc cli get-text --pane-id <N> > C:/Users/Home/p.txt   # 讀 pane (因 bash redirect bug 寫到 file 較穩)
```

### Codex 跑 cargo 時的中文路徑問題
中文路徑 `D:\興趣專案\...` 會讓 candle-cuda 編譯失敗。Codex 必走：

```powershell
# 建 ASCII junction
New-Item -ItemType Junction -Path 'C:\Users\Home\c2t-tauri' -Target 'D:\興趣專案\Github\Capture2Text 專案\rust\capture2text-pro-rs\src-tauri'

# 設 MSVC env + cargo
cmd /c 'call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" && cd /d C:\Users\Home\c2t-tauri && cargo build'

# 清 junction
cmd /c "rmdir C:\Users\Home\c2t-tauri"
```

### CUDA toolkit 環境
```
CUDA_PATH = C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v12.4
nvcc.exe = $env:CUDA_PATH\bin\nvcc.exe (12.4.131)
```

新 process 才看到（從 PATH inherit），舊 process 看不到。

### Codex CLI 版本
v0.125.0（剛升）。模型 `gpt-5.5 medium`。舊版 0.122.0 報 `model requires newer version`。

### Node.js
**Node.js 之前被 Avira 隔離**，剛重裝 v24.15.0（透過 winget OpenJS.NodeJS.LTS）。codex-cli 需要 node。

---

## 8. 重要檔案結構

```
D:\興趣專案\Github\Capture2Text 專案\rust\capture2text-pro-rs\
├── HANDOFF.md ← 本檔
├── T*_SPEC.md ← 各任務 spec（27 個）
├── T28_PLAN.md / STAGE7_PLAN.md ← 階段規劃
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── src/
│   │   ├── lib.rs ← setup spawn llama_runtime + qwen_tts
│   │   ├── main.rs
│   │   ├── llama_runtime/  ← T50a 新模組
│   │   │   ├── mod.rs
│   │   │   ├── manifest.rs
│   │   │   ├── downloader.rs
│   │   │   └── supervisor.rs
│   │   ├── qwen_tts/  ← T51a 新模組
│   │   │   ├── mod.rs
│   │   │   ├── runtime.rs
│   │   │   └── downloader.rs
│   │   ├── tts/mod.rs ← facade 呼叫 qwen_tts (改寫過)
│   │   ├── commands/
│   │   │   ├── tts.rs ← speak / list_voice_presets / set_active_preset
│   │   │   ├── result_window.rs ← +大量 set_speech_* / health / scenario / output_lang commands
│   │   │   └── translate.rs ← 7 語切換
│   │   ├── vlm/mod.rs ← 改成 OpenAI compat client to llama-server
│   │   ├── window_state.rs ← speech_active_preset / speech_*_voices / etc
│   │   ├── tray.rs ← 7 語 + 4 clip mode + scenario submenu
│   │   ├── output_lang.rs ← 7 語 sanitize
│   │   ├── capture/log.rs ← captures.log writer
│   │   ├── clipboard.rs ← arboard 寫剪貼簿
│   │   ├── app_handle.rs ← 全域 AppHandle 給 emit
│   │   └── bin/
│   │       ├── vlm_smoke.rs
│   │       └── tts_isolated_test.rs ← 隔離測試 binary
├── src/  (React)
│   ├── result/ResultView.tsx ← 移除 cacheReady, Speak 按鈕直接 invoke
│   ├── settings/
│   │   ├── SettingsView.tsx ← 4 tab + healthRetry button
│   │   ├── SettingsView.css ← spacing tokens
│   │   └── tabs/
│   │       ├── TranslateTab.tsx ← 7 語 radio + scenarios CRUD
│   │       ├── SpeechTab.tsx ← Qwen3-TTS 9 preset list
│   │       ├── OutputTab.tsx ← 4 clip mode radio + log file
│   │       └── AboutTab.tsx ← Ollama health (sic 還沒改 llama.cpp 命名) + GitHub + 匯出匯入
│   └── styles/tokens.css ← design tokens
└── %LOCALAPPDATA%\Capture2TextPro\
    ├── bin\llama-server.exe + ggml*.dll + cudart*.dll
    ├── models\
    │   ├── qwen3-vl-4b-instruct.Q4_K_M.gguf (~2.5 GB)
    │   ├── qwen3-vl-4b-instruct.mmproj.gguf
    │   ├── pixtral-12b-2409.Q4_K_M.gguf (~7.5 GB)
    │   └── pixtral-12b-2409.mmproj.gguf
    ├── tts_models\customvoice\ (Qwen3-TTS, ~1.8 GB)
    ├── window_state.json
    ├── captures.log (T38)
    └── tts_debug\*.wav (debug dump from app speak)
```

---

## 9. 關鍵 spec 參考（C:\Users\Home\）

- `T50A_SPEC.md` - llama.cpp runtime base
- `T50B_SPEC.md` - Pixtral switch + 7 語
- `T50A_HOTSWAP_4B.txt` - 8B → 4B
- `T51A_SPEC.md` - Qwen3-TTS 整合
- `T51A_OPTIONS.txt` - **下個 session 可能要重發**：SynthesisOptions 加 temperature/top_k/max_length
- `T51A_ISOLATED_TEST.md` - isolated test binary spec
- `T51A_ZH_VOICES.txt` - 4-speaker 比較 spec（**進行中**）
- `T51B_SPEC.md` - Voice cloning（後續）

---

## 10. 已知技術細節

### llama-server spawn args
```
--model <gguf>
--mmproj <mmproj.gguf>
--host 127.0.0.1
--port 11434
--n-gpu-layers 20      ← partial offload（user 接受小代償，留 VRAM 給 TTS）
--ctx-size 4096
--chat-template chatml or pixtral
--flash-attn on        ← b8870 後新格式
```

### Qwen3-TTS API（TrevorS v0.3.0 candle）
```rust
let device = qwen3_tts::auto_device()?;        // CUDA / Metal / CPU 自動
let model = Qwen3TTS::from_pretrained(&model_dir, device)?;
let opts = SynthesisOptions { temperature: 0.9, top_k: 30, max_length: 2048, ..Default::default() };
let audio = model.synthesize_with_voice(text, Speaker::Vivian, Language::Chinese, Some(opts))?;
audio.save("out.wav")?;
// 或 streaming：
for chunk in model.synthesize_streaming(text, speaker, lang, opts)? { ... }
```

### Edge TTS 已完全廢除
- `edge-tts-rust` 從 Cargo.toml 移除
- `tts/config.rs` Edge TTS 相關清空
- `commands/tts.rs` 沒了 set_tts_voice / get_tts_config / refresh_tts_voices / preview_voice

---

## 11. Git 還沒 commit 的改動

從 `7bcc70d` 起：
- **21 檔修改**
- **3042 行新增 / 1496 行刪除**
- 新增（uncommitted）：
  - `llama_runtime/`（4 檔）
  - `qwen_tts/`（3 檔）
  - `app_handle.rs`、`clipboard.rs`、`capture/log.rs`
  - `bin/tts_isolated_test.rs`
  - `voices/`（empty，T51b 會用）
  - 27 個 `T*_SPEC.md`
  - 多個 settings tab 檔
  - `HANDOFF.md`（本檔）

**建議：T51 完整通後一次 commit**（避免中間半完成 commit）。

---

## 12. 環境特殊注意事項

1. **中文路徑限制**：candle-cuda / nvcc 不支援中文 path，必走 ASCII junction。
2. **Avira 自動掃描**：可能不定時隔離 build artifacts (rustls / glutin / target/debug/build/*) 或 wezterm-cc.exe 或 node.exe。user 已加排除規則，但**新增的 path 可能仍被掃**。
3. **Node.js 容易被 Avira 隔離**：曾發生過，要重裝。
4. **Tauri dev mode**：file watcher 看 `src-tauri/src/` 改動 → 自動 rebuild + restart app。但**改 Cargo.toml dep 或 crate checkout 不會 trigger reload**，要手動 kill dev + restart。
5. **WezTerm cli send-text 對中文 byte 有 bug**：送多字節中文可能 lost 一兩個 byte。spec 必走「寫到 .txt 檔 + cx-send `cat file`」pattern。
6. **bash redirect 偶爾失敗**：`wezterm-cc cli get-text --pane-id N > /tmp/x.txt` 有時 bash 看不到 output。寫 `C:/Users/Home/p.txt` 較穩。
7. **CC↔Codex token cost 高**：Codex 跑 cargo build 每次 5-15 分鐘，等 monitor 期間 CC 常被 timeout 訊號干擾，注意辨識「真新事件 vs 舊 monitor 過期」。

---

## 13. User 個性與決策模式

- 性別：男 / 中文母語 / 自學工程師背景
- 偏好：**簡單直接**，不喜歡冗長解釋
- 決策果決，但會反覆驗證（聽多次音訊、實測多次）
- 對 UI 細節敏感（spacing / 對齊 / 卡片邊距）
- 對速度耐心低（10s+ 等待會抱怨）
- **不接受比 Edge TTS 更差的音質**
- 願意 partial offload 換穩定（小代償）
- 會主動關注資源使用（VRAM / CPU）

---

## 14. 任何時刻可立即查的單一指令

```bash
# 看 Codex pane
source ~/bin/cx-env && wezterm-cc cli list

# 讀 pane（取代 get-text 直接 stdout）
source ~/bin/cx-env && wezterm-cc cli get-text --pane-id 2 > C:/Users/Home/p.txt && tail -30 C:/Users/Home/p.txt

# 看 dev log（背景 task id 換每次）
powershell.exe -Command "Get-Content 'C:\Users\Home\AppData\Local\Temp\claude\<session-id>\tasks\<task-id>.output' -Tail 30"

# GPU 狀態
powershell.exe -Command "nvidia-smi --query-gpu=memory.used,memory.free,utilization.gpu --format=csv,noheader"

# 看當前 process
powershell.exe -Command "Get-Process -Name capture2text-pro-rs,llama-server,cargo -ErrorAction SilentlyContinue | Select-Object Id,Name"
```

---

**祝下個 session 順利交接！**
