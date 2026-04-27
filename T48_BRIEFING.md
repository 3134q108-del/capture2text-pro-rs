# Codex 新 session briefing (reboot 後 T48)

## 專案

Capture2Text Pro：Windows 桌面 OCR + 翻譯 + 朗讀工具，Tauri 2 (Rust + React) 重寫版。

- 前端：React + Vite + TypeScript (`src/`)
- 後端：Rust + Tauri 2 (`src-tauri/`)
- VLM：Ollama + `qwen3-vl:8b-instruct`
- TTS：Edge TTS (`edge-tts-rust`)
- Hotkey：Win+Q/W/E low-level keyboard hook

## 當前狀態

Stage 7 完整功能已跑通：Settings 4 tab / Tray 新結構 / Clipboard / Cross-window sync / Edge TTS 動態 voice / Ollama auto-launch (T45) / VLM keep_alive 5m (T46)。

最新穩定版：cargo check + cargo build + npm build 全過。

## 規則

1. UTF-8 NoBOM：`[System.IO.File]::WriteAllText(path, text, (New-Object System.Text.UTF8Encoding $false))`
2. 驗證後回報 `VERDICT: APPROVED` 或 `VERDICT: REVISE:<原因>`
3. 規格透過 `@T*_SPEC.md` 引用

## T48 內容

讀 `@T48_SPEC.md`：
1. `ollama_boot.rs` 改優先 CLI `ollama.exe serve`（完全背景），GUI `ollama app.exe` 降級為 fallback
2. `tokens.css` 加 spacing / radius / text-muted token
3. `SettingsView.css` + `ResultView.css` hardcoded spacing px 統一改 `var(--c2t-space-*)`

SPEC 授權直接套改。完成跑 `cargo check` + `cargo build` + `npm build`。
