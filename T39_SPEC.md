# T39 · About tab 內容

## 目標

實作 About tab 完整內容：
- App 名稱 + 版本（getVersion）
- OCR+翻譯模型名 + Ollama endpoint + [檢查連線] 按鈕
- 快捷鍵資訊（Win+Q / Win+W / Win+E）
- TTS 引擎資訊（Microsoft Edge TTS）
- 原版資訊 + 授權（Christopher Brochtrup + GPL-3）
- GitHub × 2：原版 repo + 我們 fork（先用 placeholder URL）
- 檢查更新按鈕
- [匯出設定] [匯入設定] 按鈕（用文字輸入路徑）
- 監聽 tray 發的 `settings-navigate` event，切換 activeTab 到 about

## 鎖死（MUST）

### 1. Rust 新 commands（放在 `commands/result_window.rs` 末尾）

```rust
use std::path::Path;

#[tauri::command]
pub fn check_ollama_health() -> String {
    match crate::vlm::check_health() {
        crate::vlm::HealthStatus::Healthy => "healthy".into(),
        crate::vlm::HealthStatus::OllamaDown => "daemon_down".into(),
        crate::vlm::HealthStatus::ModelMissing { model } => format!("model_missing:{model}"),
        crate::vlm::HealthStatus::Unknown(msg) => format!("unknown:{msg}"),
    }
}

#[tauri::command]
pub fn open_external_url(url: String, app: AppHandle) -> Result<(), String> {
    tauri_plugin_opener::OpenerExt::opener(&app)
        .open_url(url, None::<&str>)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn export_settings(target_dir: String) -> Result<String, String> {
    let src = dirs::data_local_dir()
        .ok_or("local appdata not found")?
        .join("Capture2TextPro");
    if !src.exists() {
        return Err("settings directory does not exist".into());
    }
    let dst = Path::new(&target_dir).join("Capture2TextPro-backup");
    std::fs::create_dir_all(&dst).map_err(|e| e.to_string())?;
    let mut count = 0;
    for entry in std::fs::read_dir(&src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        let from = entry.path();
        let to = dst.join(&name);
        if from.is_file() {
            std::fs::copy(&from, &to).map_err(|e| e.to_string())?;
            count += 1;
        }
    }
    Ok(format!("exported {count} files to {}", dst.display()))
}

#[tauri::command]
pub fn import_settings(source_dir: String) -> Result<String, String> {
    let src = Path::new(&source_dir);
    if !src.exists() || !src.is_dir() {
        return Err("source directory does not exist".into());
    }
    let dst = dirs::data_local_dir()
        .ok_or("local appdata not found")?
        .join("Capture2TextPro");
    std::fs::create_dir_all(&dst).map_err(|e| e.to_string())?;
    let mut count = 0;
    for entry in std::fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_file() {
            std::fs::copy(&from, &to).map_err(|e| e.to_string())?;
            count += 1;
        }
    }
    Ok(format!("imported {count} files to {}", dst.display()))
}

#[tauri::command]
pub async fn check_for_updates() -> Result<String, String> {
    // GitHub releases API - 先用 hardcoded repo path
    let url = "https://api.github.com/repos/3134q108-del/capture2text-pro-rs/releases/latest";
    let resp = reqwest::Client::new()
        .get(url)
        .header("User-Agent", "Capture2TextPro")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok("no_release".into());
    }
    if !resp.status().is_success() {
        return Err(format!("status {}", resp.status()));
    }
    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let tag = json.get("tag_name").and_then(|v| v.as_str()).unwrap_or("unknown");
    Ok(tag.to_string())
}
```

**注意**：
- `open_external_url` 依賴 `tauri-plugin-opener`，已在 Cargo.toml（`tauri-plugin-opener = "2"`）。需 `use tauri_plugin_opener::OpenerExt;` import
- `check_for_updates` 是 async，需要 reqwest 的 async 版本（已有 reqwest，但只用 `blocking`；需確認 reqwest 可否直接 async；如果不行，改 `std::thread::spawn + tokio::runtime::current_thread`）。**Codex 實作時自行決策**：最簡單用 `tokio::task::spawn_blocking` 包 `reqwest::blocking::Client`
- `export_settings` / `import_settings` 用文字路徑輸入（不走 dialog plugin）

### 2. `src-tauri/src/lib.rs`

invoke_handler 註冊：
- commands::result_window::check_ollama_health
- commands::result_window::open_external_url
- commands::result_window::export_settings
- commands::result_window::import_settings
- commands::result_window::check_for_updates

### 3. `src/settings/tabs/AboutTab.tsx` 完整實作

```tsx
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { useEffect, useState } from "react";

const OLLAMA_ENDPOINT = "http://localhost:11434";
const MODEL_NAME = "qwen3-vl:8b-instruct";
const UPSTREAM_URL = "https://capture2text.sourceforge.net/";
const FORK_URL = "https://github.com/3134q108-del/capture2text-pro-rs";

export default function AboutTab() {
  const [version, setVersion] = useState<string>("…");
  const [ollamaStatus, setOllamaStatus] = useState<string>("");
  const [updateStatus, setUpdateStatus] = useState<string>("");
  const [exportDir, setExportDir] = useState<string>("");
  const [importDir, setImportDir] = useState<string>("");
  const [statusMsg, setStatusMsg] = useState<string>("");

  useEffect(() => {
    void getVersion().then(setVersion).catch(() => setVersion("unknown"));
  }, []);

  async function checkOllama() {
    setOllamaStatus("檢查中…");
    try {
      const result = await invoke<string>("check_ollama_health");
      setOllamaStatus(formatOllama(result));
    } catch (err) { setOllamaStatus(`錯誤：${err}`); }
  }

  function formatOllama(code: string): string {
    if (code === "healthy") return "✓ Ollama 正常";
    if (code === "daemon_down") return "✗ Ollama daemon 未啟動";
    if (code.startsWith("model_missing:")) return `✗ 模型未安裝：${code.slice("model_missing:".length)}`;
    if (code.startsWith("unknown:")) return `⚠ 狀態不明：${code.slice("unknown:".length)}`;
    return code;
  }

  async function checkUpdate() {
    setUpdateStatus("查詢中…");
    try {
      const tag = await invoke<string>("check_for_updates");
      if (tag === "no_release") { setUpdateStatus("尚未發佈正式版"); return; }
      setUpdateStatus(`最新版本：${tag}（當前：v${version}）`);
    } catch (err) { setUpdateStatus(`查詢失敗：${err}`); }
  }

  async function openUrl(url: string) {
    try { await invoke("open_external_url", { url }); }
    catch (err) { setStatusMsg(String(err)); }
  }

  async function doExport() {
    if (!exportDir.trim()) { setStatusMsg("請先輸入匯出目錄"); return; }
    try {
      const r = await invoke<string>("export_settings", { targetDir: exportDir });
      setStatusMsg(`✓ ${r}`);
    } catch (err) { setStatusMsg(`匯出失敗：${err}`); }
  }

  async function doImport() {
    if (!importDir.trim()) { setStatusMsg("請先輸入來源目錄"); return; }
    try {
      const r = await invoke<string>("import_settings", { sourceDir: importDir });
      setStatusMsg(`✓ ${r}`);
    } catch (err) { setStatusMsg(`匯入失敗：${err}`); }
  }

  return (
    <div className="settings-translate-root">
      <section className="settings-section">
        <h2>Capture2Text Pro v{version}</h2>
        <p style={{ margin: 0, color: "var(--c2t-text-muted)" }}>
          Windows OCR + 翻譯 + 朗讀工具（Tauri + Rust 重寫版）
        </p>
      </section>

      <section className="settings-section">
        <h2>OCR + 翻譯模型</h2>
        <div>模型：<code>{MODEL_NAME}</code></div>
        <div>後端：<code>{OLLAMA_ENDPOINT}</code></div>
        <div style={{ marginTop: 6 }}>
          <button className="c2t-btn" onClick={checkOllama}>檢查 Ollama 連線</button>
          {ollamaStatus && <span style={{ marginLeft: 10 }}>{ollamaStatus}</span>}
        </div>
      </section>

      <section className="settings-section">
        <h2>快捷鍵</h2>
        <ul style={{ margin: 0, paddingLeft: 18 }}>
          <li><kbd>Win</kbd>+<kbd>Q</kbd>：框選區域擷取</li>
          <li><kbd>Win</kbd>+<kbd>W</kbd>：目前視窗擷取</li>
          <li><kbd>Win</kbd>+<kbd>E</kbd>：全螢幕擷取</li>
        </ul>
      </section>

      <section className="settings-section">
        <h2>語音引擎</h2>
        <div>Microsoft Edge TTS（雲端，免費，需網路）</div>
      </section>

      <section className="settings-section">
        <h2>原版與授權</h2>
        <div>原作者：Christopher Brochtrup</div>
        <div>授權：GPL-3.0</div>
        <div style={{ marginTop: 6, display: "flex", gap: 8 }}>
          <button className="c2t-btn" onClick={() => openUrl(UPSTREAM_URL)}>原版官網</button>
          <button className="c2t-btn" onClick={() => openUrl(FORK_URL)}>本專案 GitHub</button>
        </div>
      </section>

      <section className="settings-section">
        <h2>檢查更新</h2>
        <div>
          <button className="c2t-btn" onClick={checkUpdate}>立即查詢最新版</button>
          {updateStatus && <span style={{ marginLeft: 10 }}>{updateStatus}</span>}
        </div>
      </section>

      <section className="settings-section">
        <h2>設定匯出 / 匯入</h2>
        <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          <div>
            <label>
              匯出目錄（系統會在此建立 Capture2TextPro-backup/ 子目錄）
              <input type="text" value={exportDir} onChange={e => setExportDir(e.target.value)} placeholder="例：D:\backup" />
            </label>
            <button className="c2t-btn" style={{ marginTop: 6 }} onClick={doExport}>匯出設定</button>
          </div>
          <div>
            <label>
              匯入來源目錄
              <input type="text" value={importDir} onChange={e => setImportDir(e.target.value)} placeholder="例：D:\backup\Capture2TextPro-backup" />
            </label>
            <button className="c2t-btn" style={{ marginTop: 6 }} onClick={doImport}>匯入設定</button>
          </div>
        </div>
      </section>

      {statusMsg && <div className="settings-status">{statusMsg}</div>}
    </div>
  );
}
```

### 4. `src/settings/SettingsView.tsx` 加監聽 `settings-navigate` event

useEffect 內多一個 listener：

```tsx
const navPromise = listen<string>("settings-navigate", (event) => {
  const target = event.payload;
  if (target === "translate" || target === "speech" || target === "output" || target === "about") {
    setActiveTab(target as TabKey);
  }
});
return () => {
  unlistenPromise.then(off => off());
  navPromise.then(off => off());
};
```

## 禁動

- **不動** Popup (ResultView)
- **不動** 既有 tts / vlm / capture 模組業務邏輯
- **不動** Cargo.toml（不加新 dep，reqwest / dirs 已存在）
- **不加** dialog plugin（用文字路徑）

## 驗證

- `cargo check`
- `npm build`

## 回報

```
=== T39 套改結果 ===
- commands/result_window.rs 新 5 commands
- lib.rs 註冊 5 commands
- AboutTab.tsx 完整重寫
- SettingsView.tsx 加 settings-navigate listener
- cargo check: <結果>
- npm build: <結果>
- UTF-8 NoBOM

VERDICT: APPROVED
```

**直接套改**。check_for_updates 若 async / reqwest 卡到，改 spawn_blocking。UTF-8 NoBOM。
