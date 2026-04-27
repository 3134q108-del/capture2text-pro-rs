# T50 · Ollama → llama.cpp 置換 + 雙 VLM + 德法 output_lang

## 背景

Ollama 自家 ggml 分支導致 vision model 風險 + 效能落後 upstream。換 llama.cpp。

User 決策：
- 方向 A：output_lang 擴 7 語（加 de-DE / fr-FR）
- 主 VLM：Qwen3-VL-8B-Instruct（zh-TW/zh-CN/en-US/ja-JP/ko-KR）
- 歐語 VLM：Pixtral-12B-2409（de-DE/fr-FR）
- App 內自動下載 llama-server binary + 2 個 GGUF + 2 個 mmproj
- 切 output_lang 觸發 model reload，~15 秒等待可接受
- 兩個 model 都必須是 instruct 版（無 thinking token）

## 總規模

~1000-1500 行 Rust/TS diff，3 phase：
- **Phase 1（T50a）**：核心 runtime — llama-server spawn + downloader + VLM client 改 OpenAI 相容
- **Phase 2（T50b）**：output_lang 擴 7 語 + model switching
- **Phase 3（T50c）**：UX 收尾（About tab 文字 / Tray 7 語 / 刪 ollama_boot）

## 共通檔案結構

```
src-tauri/src/
├── llama_runtime/
│   ├── mod.rs           # 公開 API
│   ├── manifest.rs      # ModelManifest / ModelSpec
│   ├── downloader.rs    # HTTP GET with progress
│   └── supervisor.rs    # spawn/kill llama-server child process
├── vlm/mod.rs           # 改 OpenAI 相容 streaming client
├── ollama_boot.rs       # 刪除
└── commands/
    ├── result_window.rs # check_ollama_health 改 check_llm_health
    └── model.rs         # 新：download_models, switch_model, model_status
```

## 模型檔案位置

`%LOCALAPPDATA%\Capture2TextPro\`
```
├── bin/
│   ├── llama-server.exe
│   ├── ggml-*.dll, cudart64_*.dll, ...（CUDA runtime DLLs）
├── models/
│   ├── qwen3-vl-8b-instruct.Q4_K_M.gguf
│   ├── qwen3-vl-8b-instruct.mmproj.gguf
│   ├── pixtral-12b-2409.Q4_K_M.gguf
│   └── pixtral-12b-2409.mmproj.gguf
```

## 下載來源（hardcoded URL）

| 資源 | URL | 大小 (approx) |
|------|-----|---------------|
| llama.cpp Windows CUDA12 binary | `https://github.com/ggerganov/llama.cpp/releases/download/b<latest>/llama-b<latest>-bin-win-cuda12-x64.zip` | ~300 MB |
| Qwen3-VL-8B-Instruct GGUF Q4_K_M | `https://huggingface.co/unsloth/Qwen3-VL-8B-Instruct-GGUF/resolve/main/Qwen3-VL-8B-Instruct-Q4_K_M.gguf` | ~5 GB |
| Qwen3-VL-8B-Instruct mmproj | `https://huggingface.co/unsloth/Qwen3-VL-8B-Instruct-GGUF/resolve/main/mmproj-Qwen3-VL-8B-Instruct-f16.gguf` | ~1.5 GB |
| Pixtral-12B-2409 GGUF Q4_K_M | `https://huggingface.co/bartowski/pixtral-12b-GGUF/resolve/main/pixtral-12b-Q4_K_M.gguf` | ~7.5 GB |
| Pixtral-12B-2409 mmproj | `https://huggingface.co/bartowski/pixtral-12b-GGUF/resolve/main/mmproj-pixtral-12b-f16.gguf` | ~1 GB |

**Codex 實作時先 curl HEAD 驗證連結存活**；若 HF URL 有變動，log 報錯讓 user 手動放檔。

## llama-server 啟動參數

```powershell
llama-server.exe ^
  --model %LOCALAPPDATA%\Capture2TextPro\models\<current>.gguf ^
  --mmproj %LOCALAPPDATA%\Capture2TextPro\models\<current>.mmproj.gguf ^
  --host 127.0.0.1 ^
  --port 11434 ^
  --n-gpu-layers 99 ^
  --ctx-size 4096 ^
  --chat-template <qwen-vl|pixtral> ^
  --flash-attn
```

## 驗證總策略

每個 phase 完成後：
- `cargo check` + `cargo build` PASS
- `npm build` PASS
- UTF-8 NoBOM

手測等全 3 phase 完成後一次驗收。

## Phase 文件

- `T50A_SPEC.md`：Phase 1 核心 runtime
- `T50B_SPEC.md`：Phase 2 多 model 切換 + output_lang 擴 7 語
- `T50C_SPEC.md`：Phase 3 UX 收尾
