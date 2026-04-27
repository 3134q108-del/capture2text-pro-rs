# T51b · Voice Cloning (Base model) CRUD

## 目標

T51a 完成 Preset 之後，加上 **聲音克隆** 功能：user 上傳 3-10 秒 reference audio → 產生 speaker embedding → 存成 voice 檔 → 可重用。

## Model 下載

Qwen3-TTS **Base model** 從 HF 下載到 `%LOCALAPPDATA%\Capture2TextPro\tts_models\base\`。
**按需下載**：user 第一次進 Settings 點「啟用克隆功能」才觸發（跟 CustomVoice 自動下載不同）。

HF repo: `Qwen/Qwen3-TTS-12Hz-0.6B-Base`（~2 GB）。

## 架構

### 新增型別

`src-tauri/src/qwen_tts/mod.rs` 擴：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClonedVoice {
    pub id: String,          // 使用者給的名稱，唯一識別
    pub display_name: String,
    pub created_at: String,  // ISO8601
    pub language_hint: String, // "zh-TW" / "en-US" 等 (ref audio 的主要語言)
    pub embedding_path: String, // bin 檔路徑
    pub reference_transcript: Option<String>, // ICL 模式用
}

/// active voice 的 discriminator — 支援 Preset 或 Cloned
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActiveVoice {
    Preset { id: String },    // e.g. "Ryan"
    Cloned { id: String },    // e.g. "阿狗"
}
```

### `qwen_tts/cloned.rs` 新檔

```rust
use std::path::PathBuf;
use std::fs;

use super::ClonedVoice;

pub fn voices_dir() -> PathBuf {
    crate::llama_runtime::app_dir().join("voices").join("cloned")
}

/// 列所有克隆聲音 (讀 voices/cloned/*.json)
pub fn list_all() -> Vec<ClonedVoice> {
    let dir = voices_dir();
    if !dir.exists() { return Vec::new(); }
    let mut out = Vec::new();
    for entry in fs::read_dir(&dir).unwrap_or_else(|_| Vec::new().into_iter().collect::<fs::ReadDir>()).flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            if let Ok(raw) = fs::read_to_string(&path) {
                if let Ok(v) = serde_json::from_str::<ClonedVoice>(&raw) {
                    out.push(v);
                }
            }
        }
    }
    out.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    out
}

/// 儲存 (寫 <id>.json 和 <id>.bin)
pub fn save(voice: &ClonedVoice, embedding_bytes: &[u8]) -> Result<(), String> {
    let dir = voices_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let json_path = dir.join(format!("{}.json", voice.id));
    let bin_path = dir.join(format!("{}.bin", voice.id));
    fs::write(&bin_path, embedding_bytes).map_err(|e| e.to_string())?;
    let mut v = voice.clone();
    v.embedding_path = bin_path.to_string_lossy().to_string();
    let raw = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?;
    fs::write(&json_path, raw).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete(id: &str) -> Result<(), String> {
    let dir = voices_dir();
    let _ = fs::remove_file(dir.join(format!("{}.json", id)));
    let _ = fs::remove_file(dir.join(format!("{}.bin", id)));
    Ok(())
}

pub fn load_embedding(id: &str) -> Result<Vec<u8>, String> {
    let dir = voices_dir();
    fs::read(dir.join(format!("{}.bin", id))).map_err(|e| e.to_string())
}
```

### `qwen_tts/runtime.rs` 擴

```rust
static BASE_MODEL: OnceLock<Mutex<Option<Qwen3TTS>>> = OnceLock::new();

pub fn init_base() -> Result<(), String> {
    let model_dir = super::app_tts_dir().join("base");
    let device = auto_device().map_err(|e| e.to_string())?;
    let m = Qwen3TTS::from_pretrained(&model_dir, device).map_err(|e| e.to_string())?;
    let slot = BASE_MODEL.get_or_init(|| Mutex::new(None));
    if let Ok(mut g) = slot.lock() { *g = Some(m); }
    Ok(())
}

/// 從 reference audio + optional transcript 產生 speaker embedding
/// 若 transcript = Some → ICL 模式（更精準）
/// 若 None → X-vector 模式（純音訊，快）
pub fn clone_voice_embedding(
    reference_audio_path: &str,
    reference_transcript: Option<&str>,
) -> Result<Vec<u8>, String> {
    // 呼叫 qwen3-tts crate 的 create_voice_clone_prompt 或類似 API
    // 回傳 speaker embedding bytes（具體格式由 crate 定義，可能是 safetensors 或 bincode）
    // Codex 讀 crate docs 確認確切 function signature
    unimplemented!("Codex 實作")
}

/// 用克隆的 embedding 合成
pub fn synthesize_cloned_wav(
    text: &str,
    embedding_bytes: &[u8],
    lang_hint: &str,
    reference_transcript: Option<&str>,
) -> Result<Vec<u8>, String> {
    let slot = BASE_MODEL.get().ok_or("base model not loaded")?;
    let guard = slot.lock().map_err(|_| "lock poisoned")?;
    let model = guard.as_ref().ok_or("base model not loaded")?;
    // 用 synthesize_voice_clone(text, embedding_or_prompt, language, options)
    // Codex 依實際 API 調用
    unimplemented!("Codex 實作")
}
```

### `qwen_tts/downloader.rs` 擴

```rust
pub fn ensure_base_installed(on_progress: impl Fn(u64, u64) + Send + 'static) -> Result<(), String> {
    let target_dir = super::app_tts_dir().join("base");
    if base_ready(&target_dir) { return Ok(()); }
    // 下載 Qwen/Qwen3-TTS-12Hz-0.6B-Base 到 target_dir
    // 進度 emit 用 window-state-changed 類似機制發 tts-base-download-progress
    // Codex 實作
    Ok(())
}

fn base_ready(dir: &std::path::Path) -> bool {
    dir.join("config.json").exists()
}
```

### `commands/tts.rs` 新命令

```rust
#[tauri::command]
pub fn is_base_model_installed() -> bool {
    let dir = crate::qwen_tts::app_tts_dir().join("base");
    dir.join("config.json").exists()
}

#[tauri::command]
pub async fn download_base_model(app: AppHandle) -> Result<(), String> {
    let app_clone = app.clone();
    tokio::task::spawn_blocking(move || {
        crate::qwen_tts::downloader::ensure_base_installed(move |done, total| {
            use tauri::Emitter;
            let _ = app_clone.emit("tts-base-download-progress", serde_json::json!({
                "downloaded": done,
                "total": total,
                "percent": if total > 0 { done as f64 * 100.0 / total as f64 } else { 0.0 },
            }));
        })
    }).await.map_err(|e| e.to_string())??;
    // 下載完立刻 init
    crate::qwen_tts::runtime::init_base().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_cloned_voices() -> Vec<crate::qwen_tts::ClonedVoice> {
    crate::qwen_tts::cloned::list_all()
}

#[tauri::command]
pub async fn create_cloned_voice(
    id: String,
    display_name: String,
    reference_audio_path: String,
    reference_transcript: Option<String>,
    language_hint: String,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let embedding = crate::qwen_tts::runtime::clone_voice_embedding(
            &reference_audio_path,
            reference_transcript.as_deref(),
        )?;
        let voice = crate::qwen_tts::ClonedVoice {
            id: id.clone(),
            display_name,
            created_at: chrono::Local::now().to_rfc3339(),
            language_hint,
            embedding_path: String::new(),  // save() 會補
            reference_transcript,
        };
        crate::qwen_tts::cloned::save(&voice, &embedding)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn delete_cloned_voice(id: String) -> Result<(), String> {
    crate::qwen_tts::cloned::delete(&id)
}

#[tauri::command]
pub fn set_active_cloned_voice(id: String) -> Result<(), String> {
    // 檢查 voice 存在
    if !crate::qwen_tts::cloned::list_all().iter().any(|v| v.id == id) {
        return Err("cloned voice not found".into());
    }
    crate::window_state::set_speech_active_voice(format!("cloned:{}", id));
    Ok(())
}

// 修改既有 set_active_preset：改成統一 set_active_voice
#[tauri::command]
pub fn set_active_preset(id: String) -> Result<(), String> {
    use crate::qwen_tts::VoicePreset;
    if VoicePreset::from_str(&id).is_none() {
        return Err("unknown preset".into());
    }
    crate::window_state::set_speech_active_voice(format!("preset:{}", id));
    Ok(())
}
```

### `window_state.rs` 改

把 T51a 的 `speech_active_preset: String` 改成更通用的 `speech_active_voice: String`：
- 格式：`"preset:Ryan"` 或 `"cloned:阿狗"`
- Default: `"preset:Ryan"`

```rust
#[serde(default = "default_active_voice", alias = "speech_active_preset")]
pub speech_active_voice: String,

fn default_active_voice() -> String { "preset:Ryan".to_string() }
```

### `tts/mod.rs` 改 synthesize_for_active_voice

```rust
pub fn synthesize_for_active_voice(text: &str, lang: &str) -> Result<Vec<u8>, String> {
    let state = crate::window_state::get();
    let (kind, id) = match state.speech_active_voice.split_once(':') {
        Some(parts) => parts,
        None => ("preset", "Ryan"),
    };
    match kind {
        "preset" => {
            let preset = crate::qwen_tts::VoicePreset::from_str(id)
                .unwrap_or(crate::qwen_tts::VoicePreset::Ryan);
            crate::qwen_tts::synthesize(text, preset, lang)
        }
        "cloned" => {
            let emb = crate::qwen_tts::cloned::load_embedding(id)?;
            // transcript 從 json 讀
            let voices = crate::qwen_tts::cloned::list_all();
            let voice = voices.iter().find(|v| v.id == id)
                .ok_or_else(|| format!("cloned voice {} not found", id))?;
            crate::qwen_tts::runtime::synthesize_cloned_wav(
                text,
                &emb,
                lang,
                voice.reference_transcript.as_deref(),
            )
        }
        _ => Err("unknown voice kind".into()),
    }
}
```

## React UI

### `SpeechTab.tsx` 擴「克隆聲音」section

替換 T51a 的「克隆聲音 - T51b 即將提供」placeholder：

```tsx
const [baseInstalled, setBaseInstalled] = useState(false);
const [baseDownloadProgress, setBaseDownloadProgress] = useState<number | null>(null);
const [clonedVoices, setClonedVoices] = useState<ClonedVoice[]>([]);

// 在 refresh() 加：
setBaseInstalled(await invoke("is_base_model_installed"));
setClonedVoices(await invoke("list_cloned_voices"));

// listen 下載進度
useEffect(() => {
  const p = listen<{ percent: number }>("tts-base-download-progress", e => {
    setBaseDownloadProgress(e.payload.percent);
  });
  return () => { p.then(off => off()); };
}, []);

async function startBaseDownload() {
  setBaseDownloadProgress(0);
  try {
    await invoke("download_base_model");
    setBaseInstalled(true);
    setBaseDownloadProgress(null);
  } catch (err) {
    setStatusMsg(`下載失敗：${err}`);
  }
}

// UI:
<section className="settings-section">
  <h2>克隆聲音</h2>
  {!baseInstalled && baseDownloadProgress === null && (
    <div>
      <p>需要下載 Base model (~2 GB) 才能使用克隆功能。</p>
      <button className="c2t-btn" onClick={startBaseDownload}>下載並啟用</button>
    </div>
  )}
  {baseDownloadProgress !== null && (
    <div>下載中：{baseDownloadProgress.toFixed(1)}%</div>
  )}
  {baseInstalled && (
    <>
      <div className="cloned-voices-list">
        {clonedVoices.length === 0 && <div>尚無克隆聲音</div>}
        {clonedVoices.map(v => (
          <div key={v.id} className={activeVoice === `cloned:${v.id}` ? "active" : ""}>
            <span>{v.display_name}</span>
            <span className="muted">建立於 {v.created_at.slice(0, 10)}</span>
            <div>
              <button onClick={() => preview(v.id)}>試聽</button>
              <button onClick={() => setActiveCloned(v.id)}>設為使用中</button>
              <button onClick={() => deleteCloned(v.id)}>刪除</button>
            </div>
          </div>
        ))}
      </div>
      <button className="c2t-btn c2t-btn-primary" onClick={openCloneDialog}>新增克隆</button>
    </>
  )}
</section>
```

### 新增克隆 dialog

簡易版：跳 modal 含：
- 名稱輸入
- 選擇音訊檔（input type="file" accept="audio/*"）
- 選配：reference transcript（user 填 ref audio 對應的文字，為 ICL 模式；空白用 X-vector）
- 語言 hint 下拉（zh-TW / zh-CN / en-US / ja-JP / ko-KR / de-DE / fr-FR）
- [生成] 按鈕 → 呼叫 `create_cloned_voice`

音訊檔 trim 到 3-10 秒的 UI **先不做**（太複雜），user 自己確保原 clip 就 3-10 秒。或 Codex 用 `hound` crate 後端自動截前 10 秒。

## 禁動

- **不動** Preset / VLM / clipboard / hotkey
- **不自動下** Base model（user 點擊才觸發）
- **不處理** VoiceDesign（T51c 不做）

## 驗證

- cargo check + build + npm build 通過
- UTF-8 NoBOM
- 手測：
  - 首次進 Settings 語音 tab 看到「克隆聲音 · 下載並啟用」按鈕
  - 按下 → 顯示進度條 → 約幾分鐘下完
  - 下完顯示空列表 + [新增克隆] 按鈕
  - 新增 → 選 10 秒 wav/mp3 檔 → 命名 → 生成 → 出現在列表
  - 試聽 → 播出類似原音色的合成
  - 設為使用中 → Win+Q 後 Speak → 用克隆聲音念

## 回報

```
=== T51b 套改結果 ===
- qwen_tts/cloned.rs 新檔（CRUD）
- qwen_tts/runtime.rs 擴 Base model load + clone_voice_embedding + synthesize_cloned_wav
- qwen_tts/downloader.rs 擴 Base 按需下載
- commands/tts.rs 新 5 commands
- window_state speech_active_voice 統一格式
- SpeechTab.tsx 加「克隆聲音」section
- cargo check/build + npm build: <結果>
- UTF-8 NoBOM

VERDICT: APPROVED
```

## 風險

1. **qwen3-tts v0.1 voice cloning API** 具體 signature：Codex 讀 crate docs 確認 `synthesize_voice_clone` 和 `create_voice_clone_prompt` 的參數型別和回傳結構。embedding 是 `Vec<f32>` 還是自訂 struct？保存需要哪種格式？依實際調整。
2. **reference audio 格式**：crate 期望 WAV/MP3/任意？要前處理到 16kHz mono 嗎？Codex 查 + 若需要用 `hound` / `symphonia` 轉檔。
3. **Base + CustomVoice 同時載入 VRAM**：兩個 0.6B model = ~2.4 GB TTS VRAM + 3-7.5 GB VLM = 緊。若爆 → 加「active model only」邏輯（只載當前在用的 voice 需要的 model）。但實測才知道。

**直接套改**。UTF-8 NoBOM。
