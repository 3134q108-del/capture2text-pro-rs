# T44 · VLM keep_alive + 啟動 pre-warm + timeout + Slider CSS

## 真實診斷

跑 `ollama ps` 顯示**沒有任何模型載入**（header 下空白）。GPU 有 9.5GB free（RTX 4070 Ti 12GB），容量夠但模型沒常駐。

每次 Win+Q 前模型要從 disk cold load 6 GB → 需時 30-90s → 觸發 30s timeout。

**根本修法**：
1. **請求帶 keep_alive="30m"**：載入後模型常駐 30 分鐘不 unload
2. **App 啟動後 pre-warm**：背景發一個輕量 text-only chat 讓 Ollama 提前 load 模型，user 第一次 Win+Q 時已常駐
3. **Timeout 適度調升**：30s → 90s（pre-warm 沒來得及完成時的 fallback，大部分情況不會用到）

有了 pre-warm + keep_alive，正常情況下每次翻譯會是秒級（模型常駐時 inference 5-15s）。30s 原本就應該夠。

## 鎖死（MUST）

### 1. `src-tauri/src/vlm/mod.rs`：加 keep_alive

```rust
const REQUEST_TIMEOUT_MS: u64 = 90_000;  // 30s -> 90s 應付第一次 cold start fallback
const KEEP_ALIVE: &str = "30m";           // 新常數

#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    stream: bool,
    messages: Vec<OllamaMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    keep_alive: Option<String>,
}
```

`build_chat_request` 建 `OllamaChatRequest` 時加 `keep_alive: Some(KEEP_ALIVE.to_string())`。

### 2. `src-tauri/src/vlm/mod.rs`：加 pre-warm

新函式 `pub fn warmup()`：
```rust
pub fn warmup() {
    std::thread::spawn(|| {
        eprintln!("[vlm-warmup] start");
        let t0 = std::time::Instant::now();
        let client = match reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(180_000))  // warmup 給寬裕 timeout 3min
            .build() {
            Ok(c) => c,
            Err(err) => {
                eprintln!("[vlm-warmup] client build failed: {err}");
                return;
            }
        };
        let req = serde_json::json!({
            "model": OLLAMA_MODEL,
            "stream": false,
            "keep_alive": KEEP_ALIVE,
            "messages": [
                {"role": "user", "content": "hi"}
            ]
        });
        match client.post(format!("{}/api/chat", OLLAMA_ENDPOINT))
            .json(&req)
            .send() {
            Ok(resp) => {
                let ok = resp.status().is_success();
                eprintln!("[vlm-warmup] done in {}ms status={} ok={}",
                    t0.elapsed().as_millis(), resp.status(), ok);
            }
            Err(err) => {
                eprintln!("[vlm-warmup] failed: {err}");
            }
        }
    });
}
```

注意：
- `OLLAMA_ENDPOINT` 若不存在（確認 file 現況），用硬 code `"http://localhost:11434"`
- `OLLAMA_MODEL` 常數已有，重用
- warmup 是 fire-and-forget thread，**不 block setup**
- 用 `/api/chat` 帶 `keep_alive` + 輕量 text-only message，讓 Ollama load 模型常駐

### 3. `src-tauri/src/lib.rs`：setup 啟動 warmup

在 setup block，現有 `tts::config::init_voice_cache()` spawn 之後，再加：
```rust
std::thread::spawn(|| crate::vlm::warmup());
```
（如果 `warmup` 內部已 spawn thread 就不用再 spawn，直接 `crate::vlm::warmup();` 即可。Codex 依實作判斷）

另外：setup 中現有的 `[vlm] ollama health` 健檢仍保留，但**若 health check 失敗就 skip warmup**（daemon 沒起來，warmup 無意義）：
```rust
match crate::vlm::check_health() {
    crate::vlm::HealthStatus::Healthy => {
        crate::vlm::warmup();
    }
    other => { eprintln!("[vlm] skip warmup (health {:?})", other); }
}
```

### 4. `src/settings/SettingsView.css`：slider 溢出修正

**改既有 `.settings-section`**：加 `box-sizing: border-box`：

```css
.settings-section {
  border: 1px solid var(--c2t-border);
  border-radius: var(--c2t-radius);
  background: var(--c2t-bg);
  padding: 10px 12px;
  box-sizing: border-box;   /* 新增 */
}
```

**改既有 `.settings-slider-row`**：
```css
.settings-slider-row {
  display: flex;
  flex-direction: column;
  gap: 12px;
  box-sizing: border-box;
}
.settings-slider-row label {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 12px;
  box-sizing: border-box;
}
.settings-slider-row input[type="range"] {
  width: 100%;
  box-sizing: border-box;
  margin: 0;
  min-width: 0;   /* flex 子元素預設 min-width=auto 會撐出父容器 */
}
```

## 禁動

- 不改 HEALTH_TIMEOUT_SECS
- 不動 hotkey / capture / TTS / clipboard / tray 邏輯
- 不動 Cargo.toml（reqwest / serde_json 已有）

## 驗證

- `cargo check` + `cargo build`
- `npm build`
- UTF-8 NoBOM

## 風險

- `vlm::warmup()` 若在 dev 啟動時跑，第一次啟動會看到 `[vlm-warmup] start` 然後 `[vlm-warmup] done in XXXms`；若失敗可從 log 看 err
- warmup 發 `/api/chat` text-only 對 VL 模型可能怪（因為沒 image），但 Ollama 應該能接受純文字 message。若 Ollama 回 400，fallback 改發 `/api/generate` 或用 `qwen3-vl:8b` 不支援純文字時改其他 warmup 手段

## 回報

```
=== T44 套改結果 ===
- vlm/mod.rs: REQUEST_TIMEOUT_MS 30_000 -> 90_000
- vlm/mod.rs: 加 KEEP_ALIVE="30m" + OllamaChatRequest.keep_alive + build_chat_request 帶上
- vlm/mod.rs: 新 warmup() fn
- lib.rs: setup health check 成功後 call warmup()
- SettingsView.css: section + slider box-sizing + min-width
- cargo check: <結果>
- npm build: <結果>
- UTF-8 NoBOM

VERDICT: APPROVED
```

**直接套改**。UTF-8 NoBOM。
