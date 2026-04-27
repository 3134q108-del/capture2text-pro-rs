# Codex 新 session 背景（reboot 後第一次接 task）

## 專案

**Capture2Text Pro**：Windows 桌面 OCR + 翻譯 + 朗讀工具的 Tauri 2 Rust + React 重寫版。

- 前端：React + Vite + TypeScript（`src/`）
- 後端：Rust + Tauri 2（`src-tauri/`）
- VLM：本機 Ollama + `qwen3-vl:8b-instruct`（一條龍 OCR+翻譯）
- TTS：Microsoft Edge TTS（`edge-tts-rust` crate）
- Hotkey：Win+Q/W/E（WH_KEYBOARD_LL low-level hook）

## 當前狀態（Stage 7 已完成）

- Settings window 4 tab：翻譯 / 語音 / 輸出 / 關於（全繁體中文）
- Tray menu 重寫：設定 / 顯示彈窗 / 儲存剪貼簿 / 輸出語言 5 語 submenu / 關於 / 離開
- Output 語言擴 5 語：zh-TW / zh-CN / en-US / ja-JP / ko-KR
- Clipboard pipeline（arboard）：VLM success + Popup OK 都寫剪貼簿
- Cross-window sync：window-state-changed + output-language-changed event broadcast
- Dynamic Edge TTS voice list + fallback 硬 code 5 語
- Speech tab slider（音量/速度/音高）+ 5 語 voice dropdown + 試聽
- About tab：Ollama health / GitHub / 匯出匯入 / 檢查更新
- OutputTab：剪貼簿 / 顯示彈窗 / 記錄到檔案
- 診斷 eprintln 清理

上次 session 結束時 `cargo build` + `npm build` 都 PASS，可用 `cargo check` / `npm.cmd run build` 驗證現狀。

## 協作規則

1. 檔案寫入必須 **UTF-8 NoBOM**（用 `[System.IO.File]::WriteAllText(path, text, (New-Object System.Text.UTF8Encoding $false))` 強寫）
2. 所有檔案加 touched 後 **BOM 檢查**：`$bytes[0..2] -eq (0xEF,0xBB,0xBF)` 應為 False
3. 驗證：`cargo check`（src-tauri/）+ `cargo build` + `cmd /c "npm.cmd run build & echo EXITCODE=%ERRORLEVEL%"`
4. 每個 task 回報以 `VERDICT: APPROVED` 或 `VERDICT: REVISE:<原因>` 結尾
5. 新增規格：CC 會寫 `T*_SPEC.md` 並以 `@T*_SPEC.md` 引用
6. 下一個 task：`T43` — 規格在 `T43_SPEC.md`

## T43 內容

讀 `@T43_SPEC.md`：涵蓋 4 個改動
1. Clipboard mode 4 radio + Both 分隔符
2. Tray clipboard submenu + 情境 submenu
3. Slider 垂直排列（音量/速度/音高擠壓問題）
4. 繁中/簡中 voice 合併

SPEC 授權直接套改，不需要先給 diff 提案。完成後跑 cargo check + npm build 驗證。
