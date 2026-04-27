# T51a 隔離測試：純 qwen3-tts crate API 直接測

## 目的

完全 bypass 我們 app 的 wrapper（不走 streaming / 不走 React / 不走 dev hot reload），用最原始 crate API 直接合成一段 user 指定的中文，存成 WAV，user 用 media player 直接聽。

如果 **WAV 正常聽得到語音**：
→ qwen3-tts crate + model + GPU 都正常
→ 是我們 app 的 wrapper（streaming 邏輯 / SynthesisOptions / language mapping）有 bug

如果 **WAV 也是雜訊/靜音**：
→ qwen3-tts crate 在 user 的 GPU + Windows 環境本身就壞
→ 要 fork/換 crate / 換 backend

## 動作

新增 `src-tauri/src/bin/tts_isolated_test.rs`（獨立 binary）：

```rust
use anyhow::Result;
use qwen3_tts::{Qwen3TTS, Speaker, Language, SynthesisOptions, auto_device};
use std::path::PathBuf;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let model_dir = PathBuf::from(
        std::env::var("LOCALAPPDATA").unwrap_or_default()
    ).join("Capture2TextPro").join("tts_models").join("customvoice");

    println!("Loading model from: {}", model_dir.display());
    let device = auto_device()?;
    println!("Device: {:?}", device);

    let model = Qwen3TTS::from_pretrained(&model_dir.to_string_lossy(), device)?;
    println!("Model loaded.");

    let text = "在工作中，遇到问题在所难免，而问题不只是高强度的工作量。有些人面临的问题是工作太简单或过于无聊，从而很容易陷入一种 工作倦怠、提不起兴趣 的状态，最终失去工作动力和目标。";

    // 完全照 examples/tts.rs 的 options
    let options = SynthesisOptions {
        temperature: 0.9,
        top_k: 30,
        max_length: 2048,
        ..Default::default()
    };

    println!("Synthesizing with Speaker::Ryan + Language::Chinese (non-streaming)...");
    let t0 = std::time::Instant::now();
    let audio = model.synthesize_with_voice(
        text,
        Speaker::Ryan,
        Language::Chinese,
        Some(options.clone()),
    )?;
    println!("Synth done in {}ms ({} samples, {:.2}s audio)",
        t0.elapsed().as_millis(),
        audio.len(),
        audio.duration());

    let out_path = std::env::current_dir()?.join("isolated_test_ryan_zh.wav");
    audio.save(&out_path)?;
    println!("Saved: {}", out_path.display());

    // 也測 streaming 版做對比
    println!("\nNow trying streaming version...");
    let t0 = std::time::Instant::now();
    let mut all_chunks = Vec::new();
    let mut total_samples = 0;
    for (i, chunk) in model.synthesize_streaming(text, Speaker::Ryan, Language::Chinese, options)?.enumerate() {
        let chunk_audio = chunk?;
        total_samples += chunk_audio.len();
        all_chunks.push(chunk_audio);
        if i % 10 == 0 {
            println!("  chunk {i}: cumulative {} samples", total_samples);
        }
    }
    println!("Streaming done in {}ms total {} chunks {} samples",
        t0.elapsed().as_millis(),
        all_chunks.len(),
        total_samples);

    // 拼起來存
    let sample_rate = all_chunks.first().map(|a| a.sample_rate()).unwrap_or(24000);
    let mut all_samples = Vec::with_capacity(total_samples);
    for c in &all_chunks {
        all_samples.extend_from_slice(c.samples());
    }
    let merged = qwen3_tts::AudioBuffer::new(all_samples, sample_rate);
    let out_path2 = std::env::current_dir()?.join("isolated_test_ryan_zh_streaming.wav");
    merged.save(&out_path2)?;
    println!("Saved streaming version: {}", out_path2.display());

    Ok(())
}
```

**注意 API**:
- `audio.len()`、`audio.duration()`、`audio.save()`、`audio.sample_rate()`、`audio.samples()` 等具體 method 名照實際 crate 定義 adapt（Codex 讀 crate source 確認）
- `model.synthesize_with_voice` 跟 `synthesize_streaming` 的具體 signature 也照實際 adapt

## 跑法

```
cd C:\Users\Home\c2t-tauri  # 或建 junction
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
cargo build --bin tts_isolated_test
target\debug\tts_isolated_test.exe
```

兩個 WAV 會生成在 `src-tauri/` 目錄下（或 cwd）：
- `isolated_test_ryan_zh.wav`（非 streaming）
- `isolated_test_ryan_zh_streaming.wav`（streaming）

user 用 Windows Media Player / VLC 開 → 聽是否正常中文語音。

## 驗證 + 回報

- cargo build --bin tts_isolated_test PASS
- 跑 binary 完成（log 顯示 synth done + Saved）
- 兩個 WAV 路徑

回報：
- VERDICT: APPROVED
- 兩個 WAV 路徑
- crate API method 名（user 用 media player 聽完會給結果）

UTF-8 NoBOM。
