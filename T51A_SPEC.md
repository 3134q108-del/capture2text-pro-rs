# T51a · Qwen3-TTS 整合 (Preset-only) + 廢 Edge TTS

## 目標

用 Rust native `qwen3-tts` crate 換掉 `edge-tts-rust`。
本 phase 只做 **9 個 preset speaker**（Serena / Vivian / UncleFu / Ryan / Aiden / OnoAnna / Sohee / Eric / Dylan）。
Voice cloning 放 T51b，不做 VoiceDesign。

## 新增依賴

`src-tauri/Cargo.toml`：
```toml
qwen3-tts = { version = "0.1", features = ["hub"] }
# 刪除
# edge-tts-rust = "0.1.1"
```

## 檔案結構

```
src-tauri/src/
├── qwen_tts/
│   ├── mod.rs          # 公開 API：init、synthesize、preset 列表
│   ├── runtime.rs      # Qwen3TTS model load / hold / reuse
│   └── downloader.rs   # HF Hub 下載 CustomVoice 到 app_dir
├── tts/
│   ├── mod.rs          # 重寫為薄 facade 呼叫 qwen_tts
│   └── config.rs       # 廢掉 Edge TTS 相關 (available_voices / set_active_zh/en / ...)，留 stub 讓舊 import 不壞
```

## Model 儲存位置

`%LOCALAPPDATA%\Capture2TextPro\tts_models\customvoice\`
包含 `qwen3-tts` crate 期望的結構（config.json / model.safetensors / tokenizer 等）。

## Rust API

### `qwen_tts/mod.rs`

```rust
pub mod downloader;
pub mod runtime;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum VoicePreset {
    Serena,
    Vivian,
    UncleFu,
    Ryan,
    Aiden,
    OnoAnna,
    Sohee,
    Eric,
    Dylan,
}

impl VoicePreset {
    pub fn all() -> [VoicePreset; 9] {
        use VoicePreset::*;
        [Serena, Vivian, UncleFu, Ryan, Aiden, OnoAnna, Sohee, Eric, Dylan]
    }
    pub fn as_str(&self) -> &'static str { /* 對應字串 id */ }
    pub fn display_name(&self) -> &'static str {
        // 友善中文名例：Serena → "Serena（女 · 英）", Ryan → "Ryan（男 · 英）"
        // 每個 preset 標明「性別 · 主要語言」對應 Qwen3-TTS 官方描述
        // UncleFu → "UncleFu（男 · 中壯年）"
        // Sohee → "Sohee（女 · 韓）"
        // OnoAnna → "OnoAnna（女 · 日）"
        // Dylan, Eric, Aiden, Vivian 依官方 speaker card 填
    }
    pub fn from_str(s: &str) -> Option<VoicePreset> { /* ... */ }
}

/// 初始化 (首次啟動呼叫)：下載模型（若缺）+ load 到記憶體
pub fn bootstrap() -> Result<(), String> {
    downloader::ensure_customvoice_installed()?;
    runtime::init_customvoice()?;
    Ok(())
}

/// 合成 (blocking，回完整 WAV bytes 或 mp3 bytes)
/// text: 文字；preset: 9 preset 之一；lang hint: "zh" / "en" / "ja" / "ko" 用於語言偵測 (Qwen3-TTS 多語)
pub fn synthesize(text: &str, preset: VoicePreset, lang: &str) -> Result<Vec<u8>, String> {
    runtime::synthesize_wav(text, preset, lang)
}
```

### `qwen_tts/runtime.rs`

```rust
use std::sync::{Mutex, OnceLock};
use qwen3_tts::{Qwen3TTS, Speaker, Language, SynthesisOptions, auto_device};

static MODEL: OnceLock<Mutex<Option<Qwen3TTS>>> = OnceLock::new();

pub fn init_customvoice() -> Result<(), String> {
    let model_dir = super::app_tts_dir().join("customvoice");
    let device = auto_device().map_err(|e| e.to_string())?;
    let m = Qwen3TTS::from_pretrained(&model_dir, device).map_err(|e| e.to_string())?;
    let slot = MODEL.get_or_init(|| Mutex::new(None));
    if let Ok(mut g) = slot.lock() { *g = Some(m); }
    eprintln!("[qwen-tts] CustomVoice loaded");
    Ok(())
}

pub fn synthesize_wav(text: &str, preset: super::VoicePreset, lang_hint: &str) -> Result<Vec<u8>, String> {
    let slot = MODEL.get().ok_or("model not initialized")?;
    let guard = slot.lock().map_err(|_| "lock poisoned")?;
    let model = guard.as_ref().ok_or("model not loaded")?;
    let speaker = map_preset_to_speaker(preset);  // VoicePreset -> qwen3_tts::Speaker
    let language = map_lang(lang_hint);            // "zh" -> Language::Chinese etc.
    let opts = SynthesisOptions::default();

    let audio = model.synthesize_with_voice(text, speaker, language, Some(opts))
        .map_err(|e| e.to_string())?;

    // audio 物件有 save(path) 或 samples Vec<i16> — 看 crate 實際 API，export to WAV bytes 回傳
    let wav_bytes = audio.to_wav_bytes().map_err(|e| e.to_string())?;
    // 若 crate 無 to_wav_bytes，改成 samples + hound::WavWriter 寫 Vec<u8>
    Ok(wav_bytes)
}

fn map_preset_to_speaker(p: super::VoicePreset) -> Speaker {
    use super::VoicePreset::*;
    match p {
        Serena => Speaker::Serena,
        Vivian => Speaker::Vivian,
        UncleFu => Speaker::UncleFu,
        Ryan => Speaker::Ryan,
        Aiden => Speaker::Aiden,
        OnoAnna => Speaker::OnoAnna,
        Sohee => Speaker::Sohee,
        Eric => Speaker::Eric,
        Dylan => Speaker::Dylan,
    }
}

fn map_lang(hint: &str) -> Language {
    match hint {
        "zh-TW" | "zh-CN" | "zh" => Language::Chinese,
        "ja-JP" | "ja" => Language::Japanese,
        "ko-KR" | "ko" => Language::Korean,
        "de-DE" | "de" => Language::German,  // 若 enum 有
        "fr-FR" | "fr" => Language::French,  // 若 enum 有
        _ => Language::English,
    }
}
```

### `qwen_tts/downloader.rs`

```rust
use qwen3_tts::hub::ModelPaths;

pub fn ensure_customvoice_installed() -> Result<(), String> {
    let target_dir = super::app_tts_dir().join("customvoice");
    if customvoice_ready(&target_dir) {
        return Ok(());
    }
    eprintln!("[qwen-tts] downloading CustomVoice model...");
    // 使用 qwen3-tts 內建 hub downloader (它會抓 Qwen/Qwen3-TTS-12Hz-0.6B-CustomVoice)
    // 或用 reqwest 手動抓 HF files（同 llama_runtime 的 downloader pattern，共享進度 emit 機制）
    let paths = ModelPaths::download_to(&target_dir, Some("Qwen/Qwen3-TTS-12Hz-0.6B-CustomVoice"))
        .map_err(|e| e.to_string())?;
    eprintln!("[qwen-tts] downloaded to {:?}", paths);
    // 進度 emit: 用 window-state-changed 類似機制送 tts-download-progress event
    Ok(())
}

fn customvoice_ready(dir: &std::path::Path) -> bool {
    // 驗證 config.json / model.safetensors 存在
    dir.join("config.json").exists()
}
```

**若 `ModelPaths::download_to` API 跟我寫的不一樣**，Codex 依實際 crate docs.rs 調用。若 crate 沒提供「target dir」選項，改用 HF Hub cli 下載 + 複製。最糟回退：手動 reqwest 下 `config.json` / `tokenizer.json` / `model.safetensors` 幾個檔。

### `tts/mod.rs` 重寫

原本是 edge-tts-rust wrapper，改成 qwen_tts facade：

```rust
pub mod config; // 保留空殼避免舊 import 壞掉

use crate::qwen_tts::{self, VoicePreset};

pub fn init_runtime() -> Result<(), String> {
    qwen_tts::bootstrap()
}

/// 提供給 commands/tts.rs 呼叫
pub fn synthesize_for_active_voice(text: &str, lang: &str) -> Result<Vec<u8>, String> {
    let preset = current_active_preset();
    qwen_tts::synthesize(text, preset, lang)
}

pub fn current_active_preset() -> VoicePreset {
    // 從 window_state.speech_active_preset 讀；預設 Ryan
    let state = crate::window_state::get();
    VoicePreset::from_str(&state.speech_active_preset)
        .unwrap_or(VoicePreset::Ryan)
}

pub fn play_wav(bytes: &[u8]) -> Result<(), String> {
    // 用 rodio 播 WAV (之前 play_mp3 改成 play_wav 或雙支援)
    // Qwen3-TTS 輸出是 24kHz WAV
    // ...
}

pub fn stop_current() {
    // 停止 rodio sink（同 T35j 邏輯）
}

// 移除：synthesize_with_voice / synthesize_full / preprocess_for_speech 等 Edge TTS 專用
// 移除：TTS_CACHE（Qwen3-TTS 本地合成 500ms TTFB，不需 prefetch cache 了）
// 移除：TTS_SYNTH_LOCK（本地沒 Edge TTS RST 問題）
// 移除：prefetch
```

### `commands/tts.rs` 改寫

```rust
#[tauri::command]
pub fn speak(app: AppHandle, target: String, text: String, lang: String) -> Result<(), String> {
    if text.trim().is_empty() { return Ok(()); }

    // 不再 cache check，直接 synthesize + play（本地 fast）
    std::thread::spawn(move || {
        match crate::tts::synthesize_for_active_voice(&text, &lang) {
            Ok(wav) => {
                if let Err(err) = crate::tts::play_wav(&wav) {
                    eprintln!("[tts] play failed: {err}");
                }
                let _ = app.emit("tts-done", serde_json::json!({ "target": target }));
            }
            Err(err) => {
                eprintln!("[tts] synthesize failed: {err}");
                let _ = app.emit("tts-done", serde_json::json!({ "target": target, "error": err }));
            }
        }
    });
    Ok(())
}

#[tauri::command]
pub fn is_tts_cached(_text: String, _lang: String) -> bool {
    // 保留 signature 避免 React 壞，但永遠 return true（本地合成快，不需等 prefetch）
    true
}

#[tauri::command]
pub fn stop_speaking() -> Result<(), String> {
    crate::tts::stop_current();
    Ok(())
}

#[tauri::command]
pub fn list_voice_presets() -> Vec<VoicePresetInfo> {
    use crate::qwen_tts::VoicePreset;
    VoicePreset::all().iter().map(|p| VoicePresetInfo {
        id: p.as_str().to_string(),
        display_name: p.display_name().to_string(),
    }).collect()
}

#[tauri::command]
pub fn set_active_preset(id: String) -> Result<(), String> {
    use crate::qwen_tts::VoicePreset;
    if VoicePreset::from_str(&id).is_none() {
        return Err("unknown preset".into());
    }
    crate::window_state::set_speech_active_preset(id);
    Ok(())
}

// 廢棄命令 (可留空殼或刪，React 端會被刪除相關呼叫)：
// list_tts_voices / get_tts_config / set_tts_voice / preview_voice / refresh_tts_voices
// set_speech_volume / set_speech_rate / set_speech_pitch / set_speech_voice / set_speech_sample
// set_speech_enabled 保留（控制「是否啟用 TTS」）
```

### `window_state.rs` 改

新欄位：
```rust
#[serde(default = "default_active_preset")]
pub speech_active_preset: String,  // preset 字串 id，預設 "Ryan"

fn default_active_preset() -> String { "Ryan".to_string() }
```

**刪除**：
```rust
// speech_voices: HashMap<String, String>  // Edge TTS 每語 voice，不再需要
// speech_samples: HashMap<String, String>  // sample text 每語，不再需要
// speech_volume / speech_rate / speech_pitch  // Qwen3-TTS 這版不支援
```

保留 `speech_enabled`（controls whether Speak button works）。

新 setter：
```rust
pub fn set_speech_active_preset(v: String) { update(|s| s.speech_active_preset = v); }
```

### `lib.rs` setup

替換原本的 TTS runtime init：
```rust
// 刪除：
// if let Err(err) = tts::init_config_runtime() { ... }
// std::thread::spawn(|| tts::config::init_voice_cache());

// 新增：
let app_handle_tts = app.handle().clone();
std::thread::spawn(move || {
    match crate::tts::init_runtime() {
        Ok(()) => {
            eprintln!("[tts] runtime ready");
            use tauri::Emitter;
            let _ = app_handle_tts.emit("tts-ready", ());
        }
        Err(err) => {
            eprintln!("[tts] runtime init failed: {err}");
            use tauri::Emitter;
            let _ = app_handle_tts.emit("tts-init-failed", err);
        }
    }
});
```

### VLM prefetch 取消

`src-tauri/src/vlm/mod.rs` 的 `emit_vlm_event` success 分支：
- **移除** 兩個 `tts::prefetch(...)` spawn thread（不再需要預先合成）
- **保留** `append_capture` / `write_capture`（log + clipboard）

user 按 Speak 才同步合成（< 1 秒），不用 prefetch。

## React UI

### `src/settings/tabs/SpeechTab.tsx` 重寫

```tsx
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

type Preset = { id: string; display_name: string };
type WindowState = {
  speech_enabled: boolean;
  speech_active_preset: string;
};

export default function SpeechTab() {
  const [enabled, setEnabled] = useState<boolean>(true);
  const [presets, setPresets] = useState<Preset[]>([]);
  const [activePreset, setActivePreset] = useState<string>("Ryan");
  const [sampleText, setSampleText] = useState<string>("歡迎使用翻譯助理，這是聲音試聽。");
  const [statusMsg, setStatusMsg] = useState<string>("");

  useEffect(() => { void refresh(); }, []);

  async function refresh() {
    try {
      const [list, ws] = await Promise.all([
        invoke<Preset[]>("list_voice_presets"),
        invoke<WindowState>("get_window_state"),
      ]);
      setPresets(list);
      setEnabled(ws.speech_enabled);
      setActivePreset(ws.speech_active_preset);
    } catch (err) { setStatusMsg(String(err)); }
  }

  async function updateEnabled(v: boolean) {
    setEnabled(v);
    try { await invoke("set_speech_enabled", { value: v }); }
    catch (err) { setStatusMsg(String(err)); }
  }

  async function selectPreset(id: string) {
    try {
      await invoke("set_active_preset", { id });
      setActivePreset(id);
      setStatusMsg(`已切換為 ${id}`);
    } catch (err) { setStatusMsg(String(err)); }
  }

  async function previewPreset(id: string) {
    // 暫時：先 set active 再 speak
    // 更好：新 command preview_preset(id, text) 不影響 active
    try {
      await invoke("speak", { target: "translated", text: sampleText, lang: "zh-TW" });
    } catch (err) { setStatusMsg(String(err)); }
  }

  return (
    <div className="settings-translate-root">
      <section className="settings-section">
        <label className="settings-checkbox">
          <input type="checkbox" checked={enabled} onChange={e => updateEnabled(e.target.checked)} />
          啟用語音朗讀
        </label>
      </section>

      <section className="settings-section">
        <h2>使用中聲音</h2>
        <div>{activePreset}</div>
      </section>

      <section className="settings-section">
        <h2>Preset 聲音（9 個內建）</h2>
        <ul className="preset-list">
          {presets.map(p => (
            <li key={p.id} className={p.id === activePreset ? "active" : ""}>
              <span>{p.display_name}</span>
              <div style={{ display: "flex", gap: 6 }}>
                <button className="c2t-btn" onClick={() => previewPreset(p.id)}>試聽</button>
                <button className="c2t-btn c2t-btn-primary" onClick={() => selectPreset(p.id)} disabled={p.id === activePreset}>
                  {p.id === activePreset ? "使用中" : "設為使用中"}
                </button>
              </div>
            </li>
          ))}
        </ul>
      </section>

      <section className="settings-section">
        <h2>試聽文字</h2>
        <textarea value={sampleText} onChange={e => setSampleText(e.target.value)} rows={2} style={{ width: "100%" }} />
      </section>

      {statusMsg && <div className="settings-status">{statusMsg}</div>}

      <section className="settings-section" style={{ opacity: 0.5 }}>
        <h2>風格描述（未支援）</h2>
        <div>Qwen3-TTS Rust crate v0.1 尚未暴露 instructions API，待 v0.2+。</div>
      </section>

      <section className="settings-section" style={{ opacity: 0.5 }}>
        <h2>克隆聲音</h2>
        <div>T51b 即將提供。</div>
      </section>
    </div>
  );
}
```

## 禁動

- **不動** VLM / llama.cpp runtime / Pixtral
- **不動** clipboard / hotkey / capture
- 保留 `speech_enabled` 欄位不刪

## 驗證

- `cargo check` + `cargo build` 通過
- `npm build` 通過
- 第一次啟動 → bootstrap 下載 CustomVoice 1.8 GB（可透過 log 看進度）
- `[tts] runtime ready` 後，React 收到 `tts-ready` event
- Win+Q 截圖 → 譯文出 → 按 Speak → **本地合成 < 1 秒 TTFB** → 播放
- Settings 語音 tab 切換不同 preset → 試聽對比

## 回報

```
=== T51a 套改結果 ===
- Cargo.toml: +qwen3-tts, -edge-tts-rust
- 新 qwen_tts/ 模組 (mod / runtime / downloader)
- tts/mod.rs 重寫為 facade
- commands/tts.rs 簡化：speak / is_tts_cached / stop / list_voice_presets / set_active_preset
- window_state 擴 speech_active_preset，刪 speech_voices/samples/volume/rate/pitch
- lib.rs setup 改 bootstrap qwen_tts
- vlm/mod.rs 移除 prefetch
- SpeechTab.tsx 重寫（Preset list + 試聽 + 使用中切換）
- cargo check/build + npm build: <結果>
- UTF-8 NoBOM

VERDICT: APPROVED
```

## 風險

1. **qwen3-tts crate v0.1 的具體 API 可能跟 spec 略有出入** — Codex 實作前先讀 docs.rs / cargo doc 確認 API（Qwen3TTS / Speaker / Language / SynthesisOptions / audio output 格式）。差異就依實際 adapt。
2. **HF Hub 下載路徑配置**：`ModelPaths::download` 可能預設位置在 `~/.cache/huggingface`，要配置成我們的 `app_dir/tts_models/customvoice/`。若 crate 不支援 target dir，Codex 用 **手動 reqwest + HF resolve URL** 下 `config.json` / `model.safetensors` / `tokenizer.json` 等檔案（同 llama_runtime 的 downloader pattern）。
3. **Qwen3TTS model load 時間**：首次 init 在 CUDA 上 load 1.8 GB model 可能 3-5 秒。bootstrap 用 spawn thread，不 block app 啟動。
4. **VRAM 與 llama-server 共存**：Qwen3-TTS 在 CUDA:0，llama-server 也在 CUDA:0。兩者同時 ~4.5-5 GB（4B VLM + TTS 0.6B）。加 Pixtral 時 ~9 GB，緊但可。

**直接套改**。UTF-8 NoBOM。
