# T50a · Phase 1 核心 runtime（llama-server + downloader + OpenAI-compat VLM client）

## 目標

**完全取代 Ollama**。自動下載 llama-server.exe + 2 個 model + 2 個 mmproj，spawn 為 child process，VLM 改 OpenAI 相容 streaming client。

本 phase 不擴 output_lang 7 語（T50b），但**基礎設施**要支援（例如 ModelSpec 能註冊多個 model）。
本 phase 仍只 load 1 個 model（qwen3-vl）— T50b 會擴成動態切 Pixtral。

## 新檔案 `src-tauri/src/llama_runtime/`

### `mod.rs`

```rust
pub mod downloader;
pub mod manifest;
pub mod supervisor;

use std::sync::{Mutex, OnceLock};

use manifest::{ModelId, ModelSpec};

pub fn active_model() -> Option<ModelId> {
    ACTIVE_MODEL.get().and_then(|m| m.lock().ok().and_then(|g| g.clone()))
}

fn set_active_model(id: Option<ModelId>) {
    let slot = ACTIVE_MODEL.get_or_init(|| Mutex::new(None));
    if let Ok(mut g) = slot.lock() { *g = id; }
}

static ACTIVE_MODEL: OnceLock<Mutex<Option<ModelId>>> = OnceLock::new();

/// 啟動時呼叫一次：檢查 binary / models 就緒 → spawn llama-server with 預設 model
pub fn bootstrap(default_model: ModelId) -> Result<(), String> {
    ensure_binary_installed()?;
    ensure_model_installed(&default_model)?;
    supervisor::spawn_for(&default_model)?;
    set_active_model(Some(default_model));
    Ok(())
}

/// 切換到另一個 model（T50b 會用）
pub fn switch_model(target: ModelId) -> Result<(), String> {
    if active_model().as_ref() == Some(&target) { return Ok(()); }
    ensure_model_installed(&target)?;
    supervisor::stop();
    supervisor::spawn_for(&target)?;
    set_active_model(Some(target));
    Ok(())
}

fn ensure_binary_installed() -> Result<(), String> {
    let bin_dir = app_dir().join("bin");
    if !bin_dir.join("llama-server.exe").exists() {
        downloader::download_llama_binary(&bin_dir)?;
    }
    Ok(())
}

fn ensure_model_installed(id: &ModelId) -> Result<(), String> {
    let model_dir = app_dir().join("models");
    let spec = manifest::lookup(id).ok_or("unknown model id")?;
    for (url, filename) in [
        (spec.gguf_url, spec.gguf_filename()),
        (spec.mmproj_url, spec.mmproj_filename()),
    ] {
        let target = model_dir.join(filename);
        if !target.exists() {
            downloader::download_file(url, &target)?;
        }
    }
    Ok(())
}

pub fn app_dir() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("Capture2TextPro")
}
```

### `manifest.rs`

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelId {
    Qwen3Vl8bInstruct,
    Pixtral12b2409,
}

pub struct ModelSpec {
    pub id: ModelId,
    pub gguf_url: &'static str,
    pub mmproj_url: &'static str,
    pub chat_template: &'static str,  // "chatml" for Qwen / "pixtral" for Pixtral
    pub ctx_size: u32,
}

impl ModelSpec {
    pub fn gguf_filename(&self) -> &'static str {
        match self.id {
            ModelId::Qwen3Vl8bInstruct => "qwen3-vl-8b-instruct.Q4_K_M.gguf",
            ModelId::Pixtral12b2409 => "pixtral-12b-2409.Q4_K_M.gguf",
        }
    }
    pub fn mmproj_filename(&self) -> &'static str {
        match self.id {
            ModelId::Qwen3Vl8bInstruct => "qwen3-vl-8b-instruct.mmproj.gguf",
            ModelId::Pixtral12b2409 => "pixtral-12b-2409.mmproj.gguf",
        }
    }
}

const QWEN3_VL_8B: ModelSpec = ModelSpec {
    id: ModelId::Qwen3Vl8bInstruct,
    gguf_url: "https://huggingface.co/unsloth/Qwen3-VL-8B-Instruct-GGUF/resolve/main/Qwen3-VL-8B-Instruct-Q4_K_M.gguf",
    mmproj_url: "https://huggingface.co/unsloth/Qwen3-VL-8B-Instruct-GGUF/resolve/main/mmproj-Qwen3-VL-8B-Instruct-f16.gguf",
    chat_template: "chatml",
    ctx_size: 4096,
};

const PIXTRAL_12B: ModelSpec = ModelSpec {
    id: ModelId::Pixtral12b2409,
    gguf_url: "https://huggingface.co/bartowski/pixtral-12b-GGUF/resolve/main/pixtral-12b-Q4_K_M.gguf",
    mmproj_url: "https://huggingface.co/bartowski/pixtral-12b-GGUF/resolve/main/mmproj-pixtral-12b-f16.gguf",
    chat_template: "pixtral",
    ctx_size: 4096,
};

pub fn lookup(id: &ModelId) -> Option<&'static ModelSpec> {
    match id {
        ModelId::Qwen3Vl8bInstruct => Some(&QWEN3_VL_8B),
        ModelId::Pixtral12b2409 => Some(&PIXTRAL_12B),
    }
}
```

### `downloader.rs`

```rust
use std::fs;
use std::io::Write;
use std::path::Path;

const LLAMA_BINARY_URL: &str = "https://github.com/ggerganov/llama.cpp/releases/download/b4351/llama-b4351-bin-win-cuda12-x64.zip";
// ^ Codex 實作前驗證此 URL 還活，否則取 latest release 的對應 artifact

/// 下載檔案 + 寫到 target，過程中每 500ms emit 進度 event + stderr log
pub fn download_file(url: &str, target: &Path) -> Result<(), String> {
    if let Some(parent) = target.parent() { let _ = fs::create_dir_all(parent); }
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3600))
        .build()
        .map_err(|e| e.to_string())?;
    let mut resp = client.get(url).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("download {} failed: status {}", url, resp.status()));
    }
    let total = resp.content_length().unwrap_or(0);
    let mut file = fs::File::create(target).map_err(|e| e.to_string())?;
    let mut downloaded: u64 = 0;
    let mut last_report = std::time::Instant::now();
    let mut buf = [0u8; 1024 * 64];
    loop {
        let n = resp.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 { break; }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        downloaded += n as u64;
        if last_report.elapsed() >= std::time::Duration::from_millis(500) {
            report_progress(target, downloaded, total);
            last_report = std::time::Instant::now();
        }
    }
    report_progress(target, downloaded, total);
    eprintln!("[llama-runtime] downloaded {} bytes -> {}", downloaded, target.display());
    Ok(())
}

fn report_progress(target: &Path, done: u64, total: u64) {
    let pct = if total > 0 { done as f64 * 100.0 / total as f64 } else { 0.0 };
    let name = target.file_name().and_then(|s| s.to_str()).unwrap_or("?");
    eprintln!("[llama-download] {} {:.1}% ({}/{} bytes)", name, pct, done, total);
    if let Some(app) = crate::app_handle::get() {
        use tauri::Emitter;
        let _ = app.emit("model-download-progress", serde_json::json!({
            "file": name,
            "downloaded": done,
            "total": total,
            "percent": pct,
        }));
    }
}

/// 下載 + 解壓 llama.cpp Windows CUDA binary
pub fn download_llama_binary(bin_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(bin_dir).map_err(|e| e.to_string())?;
    let zip_path = bin_dir.join("llama-cuda.zip");
    download_file(LLAMA_BINARY_URL, &zip_path)?;
    // 解壓：直接呼叫 PowerShell Expand-Archive
    let output = std::process::Command::new("powershell.exe")
        .args(&["-NoProfile", "-Command", &format!(
            "Expand-Archive -Force -Path '{}' -DestinationPath '{}'",
            zip_path.display(), bin_dir.display()
        )])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!("unzip failed: {}", String::from_utf8_lossy(&output.stderr)));
    }
    let _ = fs::remove_file(&zip_path);
    // 某些 zip 會解到子目錄，嘗試把 llama-server.exe 提上來
    flatten_extract(bin_dir);
    if !bin_dir.join("llama-server.exe").exists() {
        return Err("llama-server.exe not found after extract".into());
    }
    Ok(())
}

fn flatten_extract(bin_dir: &Path) {
    // zip 內可能是 `build/bin/llama-server.exe`；把所有 .exe / .dll 移到 bin_dir 頂層
    for entry in walkdir::WalkDir::new(bin_dir).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.is_file() {
            let name = p.file_name().unwrap();
            let target = bin_dir.join(name);
            if p != target {
                let _ = fs::rename(p, &target);
            }
        }
    }
}
```

加依賴 `walkdir = "2"` 到 `Cargo.toml`。

### `supervisor.rs`

```rust
use std::process::{Child, Command};
use std::sync::{Mutex, OnceLock};

use super::manifest::{self, ModelId};
use super::app_dir;

static LLAMA_CHILD: OnceLock<Mutex<Option<Child>>> = OnceLock::new();

pub fn spawn_for(id: &ModelId) -> Result<(), String> {
    let spec = manifest::lookup(id).ok_or("unknown model")?;
    let bin = app_dir().join("bin").join("llama-server.exe");
    let model = app_dir().join("models").join(spec.gguf_filename());
    let mmproj = app_dir().join("models").join(spec.mmproj_filename());

    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let child = Command::new(&bin)
        .args(&[
            "--model", &model.to_string_lossy(),
            "--mmproj", &mmproj.to_string_lossy(),
            "--host", "127.0.0.1",
            "--port", "11434",
            "--n-gpu-layers", "99",
            "--ctx-size", &spec.ctx_size.to_string(),
            "--chat-template", spec.chat_template,
            "--flash-attn",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("spawn llama-server failed: {e}"))?;

    eprintln!("[llama-runtime] spawned pid={} for model={:?}", child.id(), id);
    let slot = LLAMA_CHILD.get_or_init(|| Mutex::new(None));
    if let Ok(mut g) = slot.lock() { *g = Some(child); }

    // Poll health 等 server ready
    poll_ready()
}

fn poll_ready() -> Result<(), String> {
    let started = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(120);
    while started.elapsed() < timeout {
        std::thread::sleep(std::time::Duration::from_millis(500));
        if let Ok(resp) = reqwest::blocking::Client::new()
            .get("http://127.0.0.1:11434/health")
            .timeout(std::time::Duration::from_secs(2))
            .send() {
            if resp.status().is_success() {
                eprintln!("[llama-runtime] ready in {}ms", started.elapsed().as_millis());
                return Ok(());
            }
        }
    }
    Err("llama-server did not become ready within 120s".into())
}

pub fn stop() {
    if let Some(slot) = LLAMA_CHILD.get() {
        if let Ok(mut g) = slot.lock() {
            if let Some(mut child) = g.take() {
                let _ = child.kill();
                let _ = child.wait();
                eprintln!("[llama-runtime] stopped");
            }
        }
    }
}

pub fn is_healthy() -> bool {
    reqwest::blocking::Client::new()
        .get("http://127.0.0.1:11434/health")
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}
```

## 改 `src-tauri/src/vlm/mod.rs`

### 移除
- `OLLAMA_ENDPOINT` / `OLLAMA_MODEL` 常數（或改名為 `LLAMA_ENDPOINT: &str = "http://127.0.0.1:11434"`，model 由 runtime 決定）
- `KEEP_ALIVE` 常數（llama-server 模型就常駐，不需要 keep_alive 欄位）
- `check_health` 改成 call `llama_runtime::supervisor::is_healthy()`
- `warmup` 可留但實作改成：發一次輕量 text chat 到 `/v1/chat/completions` 預熱

### 改 `OllamaChatRequest` / `OllamaMessage`
改名 + 格式：
```rust
#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    stream: bool,
    messages: Vec<ChatMessage>,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: Vec<ContentPart>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentPart {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
}

#[derive(Debug, Serialize)]
struct ImageUrl { url: String }
```

system prompt 則用 `{role: "system", content: vec![ContentPart::Text { text }]}`。

### 改 endpoint
- 從 `/api/chat` → `/v1/chat/completions`
- 從 NDJSON parse → SSE parse：
  ```
  line = "data: {...}\n"
  if line == "data: [DONE]" → 結束
  else parse JSON → extract choices[0].delta.content → 拼接到 accumulator
  ```

OpenAI chunk schema:
```json
{"id":"...","object":"chat.completion.chunk","choices":[{"delta":{"content":"..."},"finish_reason":null}]}
```

### 改 image base64 格式
- 從 Ollama 的 `images: [base64_string]` → OpenAI 的 `image_url: {url: "data:image/png;base64,BASE64"}`

### Model 名
- ChatRequest.model 傳 `"qwen3-vl-8b"` 或 `"local"` 都可以（llama-server 不挑 model 名，server 啟動時已決定 active model）
- 建議傳 `spec.id` 的字串表示（方便日後多 model 時 router 用）

## 改 `src-tauri/src/lib.rs`

### setup 內
替換原本的 Ollama boot flow：

**刪除**：
```rust
match crate::vlm::check_health() {
    Healthy => crate::vlm::warmup(),
    OllamaDown => crate::ollama_boot::prompt_and_launch(),
    ...
}
```

**改為**：
```rust
// 新 flow：
let app_handle_for_bootstrap = app.handle().clone();
std::thread::spawn(move || {
    if let Err(err) = crate::llama_runtime::bootstrap(crate::llama_runtime::manifest::ModelId::Qwen3Vl8bInstruct) {
        eprintln!("[llama-runtime] bootstrap failed: {err}");
        // emit setup-failed event，React 顯示錯誤
        use tauri::Emitter;
        let _ = app_handle_for_bootstrap.emit("llm-setup-failed", err);
        return;
    }
    // 預熱
    crate::vlm::warmup();
});
```

### 加 mod
- `mod llama_runtime;`
- `mod model_download;` 若有額外 commands

### 移除 mod
- `mod ollama_boot;` 刪掉整個檔 **本 phase 先保留，T50c 才刪避免編譯斷掉**；改成 `#[allow(dead_code)]` 註解即可

## 改 `Cargo.toml`

加：
- `walkdir = "2"`
- （reqwest 已有 blocking feature）

## 禁動

- **不擴** output_lang 7 語（T50b）
- **不刪** About tab / ollama_boot.rs（T50c）
- **不改** tray menu 結構（T50c）
- **不改** 下載 UI（本 phase 只 eprintln log 進度 + emit event，UI 之後接）

## 驗證

- `cargo check` + `cargo build` 通過
- `npm build` 通過
- **手測**（第一次啟動）：
  - 啟動 app 後看到 `[llama-runtime] downloaded ... bytes` log for binary + 2 檔
  - 然後 `[llama-runtime] spawned pid=... for model=Qwen3Vl8bInstruct`
  - 然後 `[llama-runtime] ready in Nms`
  - Win+Q 截圖 → 翻譯正常運作
  - `ollama ps` 應該是空的（Ollama 沒在跑）

## 回報

```
=== T50a 套改結果 ===
- 新 llama_runtime/ 模組（mod / manifest / downloader / supervisor）
- Cargo.toml 加 walkdir
- vlm/mod.rs 改 OpenAI 相容 API + SSE parse
- lib.rs setup 改 bootstrap llama_runtime
- ollama_boot.rs 保留 #[allow(dead_code)]（T50c 才刪）
- cargo check / cargo build: <結果>
- npm build: <結果>
- UTF-8 NoBOM

VERDICT: APPROVED
```

## 風險

1. **llama.cpp binary URL 可能變動**：Codex 實作前先 HEAD 確認 `b4351` 存在，若 404 改取 latest release 的 `llama-b*-bin-win-cuda12-x64.zip`
2. **HF URL 可能變動**：Codex 實作前先 curl HEAD 驗證 5 個 URL
3. **Pixtral GGUF 可用性**：`bartowski/pixtral-12b-GGUF` 可能尚未存在或檔名不同；若找不到，fallback 改指向 `mradermacher/pixtral-12b-i1-GGUF` 或類似
4. **CUDA 版本**：user RTX 4070 Ti 要 CUDA 12；確認 binary artifact name 正確
5. **llama-server `--chat-template pixtral`**：最新 llama.cpp 才支援，舊版沒有。選 binary 時挑 b4351+
6. **reqwest SSE streaming**：目前用 blocking + BufRead 一行一行讀即可；不需換 async

**直接套改**，但**本 phase 有外部依賴（URL/檔案可用性）**，Codex 實作前要先做 HEAD 驗證並回報。

UTF-8 NoBOM。
