# T37 · Speech tab 完整內容

## 背景

Speech tab 目前是 T28b 搬過來的舊 TTS Voice dropdown（只 2 語）。
T37 要擴成完整版：Enable / Volume / Rate / Pitch sliders + 5 語 voice dropdown + 可編 sample + 試聽按鈕。
同時要 **動態抓 Edge TTS voice list**（失敗 fallback 到 hard-coded）。

## 目標

重寫 `src/settings/tabs/SpeechTab.tsx` 為完整 UI；
擴 `window_state.rs` Schema + setters；
擴 `tts/mod.rs` + `commands/tts.rs` 加 preview_voice；
在 `tts/config.rs` 加 refresh_voice_list（動態抓）+ fallback 硬 coded 5 語。

## 鎖死（MUST）

### 1. `src-tauri/src/window_state.rs`

`WindowState` 加新欄位（serde 用 `#[serde(default)]` 保舊檔相容）：
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowState {
    // ... 既有欄位 ...

    #[serde(default = "default_speech_enabled")]
    pub speech_enabled: bool,
    #[serde(default = "default_speech_volume")]
    pub speech_volume: u32,   // 0-100
    #[serde(default = "default_speech_rate")]
    pub speech_rate: i32,     // -50 ~ +50
    #[serde(default = "default_speech_pitch")]
    pub speech_pitch: i32,    // -50 ~ +50
    #[serde(default = "default_speech_voices")]
    pub speech_voices: std::collections::HashMap<String, String>,  // "zh-TW" -> "zh-TW-HsiaoChenNeural", etc.
    #[serde(default = "default_speech_samples")]
    pub speech_samples: std::collections::HashMap<String, String>,
}

fn default_speech_enabled() -> bool { true }
fn default_speech_volume() -> u32 { 80 }
fn default_speech_rate() -> i32 { 0 }
fn default_speech_pitch() -> i32 { 0 }
fn default_speech_voices() -> std::collections::HashMap<String, String> {
    let mut m = std::collections::HashMap::new();
    m.insert("zh-TW".into(), "zh-TW-HsiaoChenNeural".into());
    m.insert("zh-CN".into(), "zh-CN-XiaoxiaoNeural".into());
    m.insert("en-US".into(), "en-US-AvaNeural".into());
    m.insert("ja-JP".into(), "ja-JP-NanamiNeural".into());
    m.insert("ko-KR".into(), "ko-KR-SunHiNeural".into());
    m
}
fn default_speech_samples() -> std::collections::HashMap<String, String> {
    let mut m = std::collections::HashMap::new();
    m.insert("zh-TW".into(), "歡迎使用翻譯助理，這是聲音試聽。".into());
    m.insert("zh-CN".into(), "欢迎使用翻译助理，这是声音试听。".into());
    m.insert("en-US".into(), "Hello, this is a voice preview.".into());
    m.insert("ja-JP".into(), "こんにちは、音声のプレビューです。".into());
    m.insert("ko-KR".into(), "안녕하세요, 음성 미리듣기입니다.".into());
    m
}
```

同樣更新 `Default for WindowState` 把這些欄位補起來。

### 2. `src-tauri/src/window_state.rs` 新 setters

```rust
pub fn set_speech_enabled(v: bool) { update(|s| s.speech_enabled = v); }
pub fn set_speech_volume(v: u32) { update(|s| s.speech_volume = v.min(100)); }
pub fn set_speech_rate(v: i32) { update(|s| s.speech_rate = v.clamp(-50, 50)); }
pub fn set_speech_pitch(v: i32) { update(|s| s.speech_pitch = v.clamp(-50, 50)); }
pub fn set_speech_voice(lang: String, code: String) {
    update(|s| { s.speech_voices.insert(lang, code); });
}
pub fn set_speech_sample(lang: String, text: String) {
    update(|s| { s.speech_samples.insert(lang, text); });
}
```

### 3. `src-tauri/src/commands/result_window.rs` 新 commands

```rust
#[tauri::command]
pub fn set_speech_enabled(value: bool) -> Result<(), String> {
    window_state::set_speech_enabled(value); Ok(())
}

#[tauri::command]
pub fn set_speech_volume(value: u32) -> Result<(), String> {
    window_state::set_speech_volume(value); Ok(())
}

#[tauri::command]
pub fn set_speech_rate(value: i32) -> Result<(), String> {
    window_state::set_speech_rate(value); Ok(())
}

#[tauri::command]
pub fn set_speech_pitch(value: i32) -> Result<(), String> {
    window_state::set_speech_pitch(value); Ok(())
}

#[tauri::command]
pub fn set_speech_voice(lang: String, code: String) -> Result<(), String> {
    if lang.trim().is_empty() || code.trim().is_empty() {
        return Err("lang and code required".into());
    }
    window_state::set_speech_voice(lang, code); Ok(())
}

#[tauri::command]
pub fn set_speech_sample(lang: String, text: String) -> Result<(), String> {
    if lang.trim().is_empty() {
        return Err("lang required".into());
    }
    window_state::set_speech_sample(lang, text); Ok(())
}
```

### 4. `src-tauri/src/tts/config.rs`

加動態抓 voice list 的機制：

```rust
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

static VOICE_CACHE: OnceLock<Mutex<Vec<TtsVoiceOption>>> = OnceLock::new();

// 擴 TtsVoiceOption 的 lang 欄位支援 "zh-TW" / "zh-CN" / "en-US" / "ja-JP" / "ko-KR"
// （現有是 "zh" / "en"，繼續用直到動態 fetch 成功）

pub fn cached_voices() -> Vec<TtsVoiceOption> {
    let slot = VOICE_CACHE.get_or_init(|| Mutex::new(fallback_voices()));
    match slot.lock() {
        Ok(g) => g.clone(),
        Err(_) => fallback_voices(),
    }
}

pub fn set_cached_voices(list: Vec<TtsVoiceOption>) {
    let slot = VOICE_CACHE.get_or_init(|| Mutex::new(fallback_voices()));
    if let Ok(mut g) = slot.lock() {
        *g = list;
    }
}

// 改名：available_voices() → fallback_voices()
// 回傳寫死 5 語的預設 list（每語 3 個）
pub fn fallback_voices() -> Vec<TtsVoiceOption> {
    vec![
        // zh-TW
        TtsVoiceOption { code: "zh-TW-HsiaoChenNeural".into(), display_name: "HsiaoChen (女)".into(), lang: "zh-TW".into() },
        TtsVoiceOption { code: "zh-TW-HsiaoYuNeural".into(), display_name: "HsiaoYu (女)".into(), lang: "zh-TW".into() },
        TtsVoiceOption { code: "zh-TW-YunJheNeural".into(), display_name: "YunJhe (男)".into(), lang: "zh-TW".into() },
        // zh-CN
        TtsVoiceOption { code: "zh-CN-XiaoxiaoNeural".into(), display_name: "Xiaoxiao (女)".into(), lang: "zh-CN".into() },
        TtsVoiceOption { code: "zh-CN-YunxiNeural".into(), display_name: "Yunxi (男)".into(), lang: "zh-CN".into() },
        TtsVoiceOption { code: "zh-CN-XiaoyiNeural".into(), display_name: "Xiaoyi (女)".into(), lang: "zh-CN".into() },
        // en-US
        TtsVoiceOption { code: "en-US-AvaNeural".into(), display_name: "Ava (女)".into(), lang: "en-US".into() },
        TtsVoiceOption { code: "en-US-AndrewNeural".into(), display_name: "Andrew (男)".into(), lang: "en-US".into() },
        TtsVoiceOption { code: "en-US-EmmaNeural".into(), display_name: "Emma (女)".into(), lang: "en-US".into() },
        // ja-JP
        TtsVoiceOption { code: "ja-JP-NanamiNeural".into(), display_name: "Nanami (女)".into(), lang: "ja-JP".into() },
        TtsVoiceOption { code: "ja-JP-KeitaNeural".into(), display_name: "Keita (男)".into(), lang: "ja-JP".into() },
        // ko-KR
        TtsVoiceOption { code: "ko-KR-SunHiNeural".into(), display_name: "SunHi (女)".into(), lang: "ko-KR".into() },
        TtsVoiceOption { code: "ko-KR-InJoonNeural".into(), display_name: "InJoon (男)".into(), lang: "ko-KR".into() },
    ]
}

// 保留 `available_voices()` 名字當 backward-compatible alias：
pub fn available_voices() -> Vec<TtsVoiceOption> {
    cached_voices()
}
```

**動態抓 voice list（fetch_remote_voices）**：

```rust
pub fn fetch_remote_voices() -> Result<Vec<TtsVoiceOption>, String> {
    // Edge TTS endpoint（不需 auth token，用 trusted client token）
    let url = "https://speech.platform.bing.com/consumer/speech/synthesize/readaloud/voices/list?trustedclienttoken=6A5AA1D4EAFF4E9FB37E23D68491D6F4";
    let resp = reqwest::blocking::Client::new()
        .get(url)
        .header("User-Agent", "Mozilla/5.0")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .map_err(|err| format!("fetch voices failed: {err}"))?;
    if !resp.status().is_success() {
        return Err(format!("voice list http status: {}", resp.status()));
    }
    let json: serde_json::Value = resp.json().map_err(|err| format!("parse json failed: {err}"))?;
    let arr = json.as_array().ok_or("voice list not an array")?;
    let mut out = Vec::new();
    for item in arr {
        let short_name = item.get("ShortName").and_then(|v| v.as_str()).unwrap_or("");
        let locale = item.get("Locale").and_then(|v| v.as_str()).unwrap_or("");
        let gender = item.get("Gender").and_then(|v| v.as_str()).unwrap_or("");
        let friendly = item.get("FriendlyName").and_then(|v| v.as_str()).unwrap_or(short_name);
        if !["zh-TW", "zh-CN", "en-US", "ja-JP", "ko-KR"].contains(&locale) {
            continue;  // 只收 5 語
        }
        // display_name 格式："{name_short}（{性別繁中}）"
        // name_short 從 ShortName 拆（"zh-TW-HsiaoChenNeural" → "HsiaoChen"）
        let name_short = short_name
            .strip_prefix(&format!("{}-", locale))
            .and_then(|s| s.strip_suffix("Neural"))
            .unwrap_or(friendly);
        let gender_cn = match gender {
            "Female" => "女",
            "Male" => "男",
            _ => gender,
        };
        out.push(TtsVoiceOption {
            code: short_name.to_string(),
            display_name: format!("{} ({})", name_short, gender_cn),
            lang: locale.to_string(),
        });
    }
    if out.is_empty() {
        return Err("no matching voices from remote".to_string());
    }
    Ok(out)
}

pub fn init_voice_cache() {
    let voices = match fetch_remote_voices() {
        Ok(list) => {
            eprintln!("[tts-voices] fetched {} voices from remote", list.len());
            list
        }
        Err(err) => {
            eprintln!("[tts-voices] remote fetch failed, fallback: {}", err);
            fallback_voices()
        }
    };
    set_cached_voices(voices);
}
```

### 5. `src-tauri/src/tts/mod.rs` 擴 preview 和 rate/pitch/volume

**新 command 對應的 wrapper（提供 rate/pitch/volume 的 options）**：

```rust
/// 簡單封裝：帶 rate/pitch/volume 的 synthesis。若 edge-tts-rust SpeakOptions 無對應欄位則忽略。
pub fn synthesize_full(text: &str, voice_code: &str, rate: i32, pitch: i32, volume: u32) -> Result<Vec<u8>, TtsError> {
    // 先保留這個函式簽名，內部實作呼叫 synthesize_with_voice（暫時忽略 rate/pitch/volume，等確認 edge-tts-rust SpeakOptions）。
    // 確認 SpeakOptions 有 rate/pitch/volume 欄位後，套到 options。
    // 若沒有（edge-tts-rust 0.1.1 限制），記錄 eprintln 並退回 synthesize_with_voice。
    let _ = (rate, pitch, volume);
    synthesize_with_voice(text, voice_code)
}

pub fn preview_voice(text: &str, voice_code: &str, rate: i32, pitch: i32, volume: u32) -> Result<(), TtsError> {
    if text.trim().is_empty() { return Err(TtsError::EmptyText); }
    let bytes = synthesize_full(text, voice_code, rate, pitch, volume)?;
    play_mp3(&bytes)
}
```

（Codex 套改時應先看 edge-tts-rust 0.1.1 的 SpeakOptions struct 欄位；如果有 `rate` / `pitch` / `volume` 就直接套用並移除上方的「暫時忽略」註解；如果沒有就保留現狀加 eprintln 警告）。

### 6. `src-tauri/src/commands/tts.rs` 新 command

```rust
#[tauri::command]
pub fn preview_voice(voice_code: String, text: String) -> Result<(), String> {
    let ws = crate::window_state::get();
    // 讀目前 slider 值當 preview 用
    std::thread::spawn(move || {
        if let Err(err) = crate::tts::preview_voice(&text, &voice_code, ws.speech_rate, ws.speech_pitch, ws.speech_volume) {
            eprintln!("[tts-preview] failed: {err}");
        }
    });
    Ok(())
}

#[tauri::command]
pub fn refresh_tts_voices() -> Result<Vec<crate::tts::TtsVoiceOption>, String> {
    match crate::tts::config::fetch_remote_voices() {
        Ok(list) => {
            crate::tts::config::set_cached_voices(list.clone());
            Ok(list)
        }
        Err(err) => Err(err),
    }
}
```

### 7. `src-tauri/src/lib.rs`

- setup 裡 `tts::init_config_runtime()` 之後加 `tts::config::init_voice_cache();`
- invoke_handler 註冊新 6 個 `set_speech_*` commands + `commands::tts::preview_voice` + `commands::tts::refresh_tts_voices`

### 8. `src/settings/tabs/SpeechTab.tsx` **完整重寫**

```tsx
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";

type TtsVoice = { code: string; display_name: string; lang: string };
type WindowState = {
  speech_enabled: boolean;
  speech_volume: number;
  speech_rate: number;
  speech_pitch: number;
  speech_voices: Record<string, string>;
  speech_samples: Record<string, string>;
};

const LANGS: { code: string; label: string }[] = [
  { code: "zh-TW", label: "繁體中文" },
  { code: "zh-CN", label: "簡體中文" },
  { code: "en-US", label: "英文" },
  { code: "ja-JP", label: "日文" },
  { code: "ko-KR", label: "韓文" },
];

export default function SpeechTab() {
  const [enabled, setEnabled] = useState<boolean>(true);
  const [volume, setVolume] = useState<number>(80);
  const [rate, setRate] = useState<number>(0);
  const [pitch, setPitch] = useState<number>(0);
  const [voices, setVoices] = useState<TtsVoice[]>([]);
  const [perLangVoice, setPerLangVoice] = useState<Record<string,string>>({});
  const [perLangSample, setPerLangSample] = useState<Record<string,string>>({});
  const [statusMsg, setStatusMsg] = useState<string>("");

  const voicesByLang = useMemo(() => {
    const map: Record<string, TtsVoice[]> = {};
    voices.forEach(v => { (map[v.lang] ??= []).push(v); });
    return map;
  }, [voices]);

  useEffect(() => { void refresh(); }, []);

  async function refresh() {
    try {
      const [list, ws] = await Promise.all([
        invoke<TtsVoice[]>("list_tts_voices"),
        invoke<WindowState>("get_window_state"),
      ]);
      setVoices(list);
      setEnabled(ws.speech_enabled);
      setVolume(ws.speech_volume);
      setRate(ws.speech_rate);
      setPitch(ws.speech_pitch);
      setPerLangVoice(ws.speech_voices ?? {});
      setPerLangSample(ws.speech_samples ?? {});
    } catch (err) { setStatusMsg(String(err)); }
  }

  async function handleRefreshVoices() {
    setStatusMsg("正在抓取最新語音清單…");
    try {
      const list = await invoke<TtsVoice[]>("refresh_tts_voices");
      setVoices(list);
      setStatusMsg(`已更新語音清單（${list.length} 個）。`);
    } catch (err) { setStatusMsg(`抓取失敗：${err}`); }
  }

  // 通用 setter with debounce-free direct save
  async function updateEnabled(v: boolean) {
    setEnabled(v);
    try { await invoke("set_speech_enabled", { value: v }); }
    catch (err) { setStatusMsg(String(err)); }
  }
  async function updateVolume(v: number) {
    setVolume(v);
    try { await invoke("set_speech_volume", { value: v }); }
    catch (err) { setStatusMsg(String(err)); }
  }
  async function updateRate(v: number) {
    setRate(v);
    try { await invoke("set_speech_rate", { value: v }); }
    catch (err) { setStatusMsg(String(err)); }
  }
  async function updatePitch(v: number) {
    setPitch(v);
    try { await invoke("set_speech_pitch", { value: v }); }
    catch (err) { setStatusMsg(String(err)); }
  }
  async function updateVoice(lang: string, code: string) {
    setPerLangVoice(prev => ({ ...prev, [lang]: code }));
    try { await invoke("set_speech_voice", { lang, code }); }
    catch (err) { setStatusMsg(String(err)); }
  }
  async function updateSample(lang: string, text: string) {
    setPerLangSample(prev => ({ ...prev, [lang]: text }));
    try { await invoke("set_speech_sample", { lang, text }); }
    catch (err) { setStatusMsg(String(err)); }
  }
  async function previewLang(lang: string) {
    const code = perLangVoice[lang];
    const text = perLangSample[lang];
    if (!code || !text) { setStatusMsg("請先選 voice 和填 sample"); return; }
    try {
      await invoke("preview_voice", { voiceCode: code, text });
      setStatusMsg(`試聽中：${lang}`);
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
        <h2>音量 / 速度 / 音高</h2>
        <div className="settings-slider-row">
          <label>音量 ({volume})
            <input type="range" min={0} max={100} step={1} value={volume} onChange={e => updateVolume(Number(e.target.value))} />
          </label>
          <label>速度 ({rate > 0 ? `+${rate}` : rate})
            <input type="range" min={-50} max={50} step={1} value={rate} onChange={e => updateRate(Number(e.target.value))} />
          </label>
          <label>音高 ({pitch > 0 ? `+${pitch}` : pitch})
            <input type="range" min={-50} max={50} step={1} value={pitch} onChange={e => updatePitch(Number(e.target.value))} />
          </label>
        </div>
      </section>

      <section className="settings-section">
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <h2>各語言語音</h2>
          <button className="c2t-btn" onClick={handleRefreshVoices}>重新抓取清單</button>
        </div>
        {LANGS.map(lang => {
          const langVoices = voicesByLang[lang.code] ?? [];
          const currentVoice = perLangVoice[lang.code] ?? "";
          const currentSample = perLangSample[lang.code] ?? "";
          return (
            <div key={lang.code} className="settings-voice-row">
              <strong style={{ minWidth: 72 }}>{lang.label}</strong>
              <select value={currentVoice} onChange={e => updateVoice(lang.code, e.target.value)}>
                {langVoices.length === 0 && <option value="">（無可用 voice）</option>}
                {langVoices.map(v => (
                  <option key={v.code} value={v.code}>{v.display_name}</option>
                ))}
              </select>
              <input type="text"
                value={currentSample}
                onChange={e => updateSample(lang.code, e.target.value)}
                placeholder="試聽文字" />
              <button className="c2t-btn" onClick={() => previewLang(lang.code)}>▶</button>
            </div>
          );
        })}
      </section>

      {statusMsg && <div className="settings-status">{statusMsg}</div>}
    </div>
  );
}
```

### 9. `src/settings/SettingsView.css` 加

```css
.settings-slider-row {
  display: grid;
  grid-template-columns: 1fr 1fr 1fr;
  gap: 12px;
}
.settings-slider-row label {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 12px;
}
.settings-slider-row input[type="range"] { width: 100%; }

.settings-voice-row {
  display: grid;
  grid-template-columns: 72px 160px 1fr auto;
  gap: 8px;
  align-items: center;
  margin-top: 6px;
}
```

## 禁動

- **不動** 其他 tab 檔
- **不動** 已存在的 speak / stop / is_tts_cached 等 command
- **不動** Popup 的 TTS 流程（ResultView）— 它繼續用 synthesize_with_voice，rate/pitch/volume 暫不生效

## 驗證

- `cargo check` + `cargo build` 通過
- `npm.cmd run build` 通過
- 不需要手測（最後一次一起測）

## 非目標

- 不實作 SSML rate/pitch/volume 實際套用（edge-tts-rust 0.1.1 可能不支援；若發現 SpeakOptions 有 rate/pitch/volume 欄位才套）
- 不做 tray ↔ settings 同步（T41）

## 風險點

1. **edge-tts-rust 0.1.1 SpeakOptions 結構未知**：Codex 要先查 lib source（`cargo doc` 或讀 `~/.cargo/registry/src/index.crates.io-.../edge-tts-rust-0.1.1/src/`）。若 `SpeakOptions` 有 `rate/pitch/volume` field → 串上去；若沒有 → 維持現狀並加 eprintln。

2. **HashMap 在 WindowState serde**：加 `#[serde(default = "...")]` 確保舊 `window_state.json`（沒 speech_\* 欄位）載入不出錯。

3. **Edge TTS voice list API 呼叫**：要 block run（reqwest::blocking），call 在 `init_voice_cache` 或 `refresh_tts_voices` 命中；setup 階段非同步 spawn 免得啟動變慢：
   ```rust
   // lib.rs setup 裡：
   std::thread::spawn(|| tts::config::init_voice_cache());
   ```

## 回報

```
=== T37 套改結果 ===
- window_state.rs 擴 speech_* 6 欄位 + 6 setters
- commands/result_window.rs 新 6 commands（set_speech_*）
- tts/config.rs 擴：fallback_voices 5 語 + fetch_remote_voices + set/get cached
- tts/mod.rs 擴：preview_voice + synthesize_full（rate/pitch/volume 串接現狀）
- commands/tts.rs 新 2 commands：preview_voice + refresh_tts_voices
- lib.rs：setup spawn init_voice_cache + 註冊 8 個新 commands
- SpeechTab.tsx 完整重寫
- SettingsView.css 加 2 組 class
- cargo check: <結果>
- npm build: <結果>
- UTF-8 NoBOM 驗證

VERDICT: APPROVED
```

**直接套改，不需要先給 diff 提案**（已詳盡規格）。全部 UTF-8 NoBOM。
實作 SpeakOptions rate/pitch/volume 前先查 edge-tts-rust 實際定義，回報發現。
