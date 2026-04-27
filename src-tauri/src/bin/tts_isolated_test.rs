use qwen3_tts::{auto_device, Language, Qwen3TTS, Speaker, SynthesisOptions};
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant, SystemTime};

#[derive(Debug, Clone, Copy)]
struct BenchSample {
    id: &'static str,
    lang_code: &'static str,
    language: Language,
    text: &'static str,
    output: &'static str,
}

#[derive(Debug, Clone)]
struct BenchResult {
    id: String,
    speaker: String,
    lang_code: String,
    rtf: f64,
    elapsed_ms: f64,
    audio_duration_s: f64,
    samples: usize,
    output: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let mut args = std::env::args().skip(1);
    if let Some(flag) = args.next() {
        if flag == "--sample" {
            let sample_id = args.next().ok_or("missing sample id")?;
            return run_one(&sample_id);
        }
        return Err(format!("unknown argument: {flag}").into());
    }

    run_all()
}

fn run_all() -> Result<(), Box<dyn std::error::Error>> {
    let exe = std::env::current_exe()?;
    let mut results = Vec::new();

    for sample in samples() {
        let mut last_error = None;
        for attempt in 1..=3 {
            println!();
            println!(
                "=== Running isolated child for sample {} (attempt {attempt}/3) ===",
                sample.id
            );
            let output = Command::new(&exe).arg("--sample").arg(sample.id).output()?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            print!("{stdout}");
            eprint!("{stderr}");

            if output.status.success() {
                if let Some(result) = parse_result_line(&stdout) {
                    results.push(result);
                    last_error = None;
                    break;
                }
                last_error = Some(format!("sample {} completed without RESULT line", sample.id));
            } else {
                last_error = Some(format!(
                    "sample {} failed with status {}",
                    sample.id, output.status
                ));
            }

            println!("sample {} failed; waiting 60s before retry...", sample.id);
            std::thread::sleep(Duration::from_secs(60));
        }

        if let Some(error) = last_error {
            return Err(format!("{error} after 3 attempts").into());
        }

        std::thread::sleep(Duration::from_secs(60));
    }

    print_summary(&results);

    Ok(())
}

fn run_one(sample_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let sample = samples()
        .into_iter()
        .find(|sample| sample.id == sample_id)
        .ok_or_else(|| format!("unknown sample id: {sample_id}"))?;

    let model_dir = PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_default())
        .join("Capture2TextPro")
        .join("tts_models")
        .join("customvoice");

    println!("Loading model from: {}", model_dir.display());
    let device = auto_device()?;
    println!("Device: {}", qwen3_tts::device_info(&device));

    let model = Qwen3TTS::from_pretrained(model_dir.to_string_lossy().as_ref(), device)?;
    println!("Model loaded.");

    let options = SynthesisOptions {
        temperature: 0.7,
        top_k: 50,
        top_p: 0.9,
        repetition_penalty: 1.05,
        max_length: 2048,
        ..Default::default()
    };

    synthesize_sample(&model, &sample, &options)
}

fn samples() -> Vec<BenchSample> {
    vec![
        BenchSample {
            id: "en",
            lang_code: "en",
            language: Language::English,
            text: "The quick brown fox jumps over the lazy dog. Modern compilers leverage SIMD instructions to accelerate vector operations. This implementation requires careful memory management for optimal performance.",
            output: "isolated_test_ryan_en.wav",
        },
        BenchSample {
            id: "de",
            lang_code: "de",
            language: Language::German,
            text: "Der schnelle braune Fuchs springt ueber den faulen Hund. Moderne Compiler verwenden SIMD-Anweisungen zur Beschleunigung von Vektoroperationen. Diese Implementierung erfordert sorgfaeltige Speicherverwaltung.",
            output: "isolated_test_ryan_de.wav",
        },
        BenchSample {
            id: "fr",
            lang_code: "fr",
            language: Language::French,
            text: "Le rapide renard brun saute par-dessus le chien paresseux. Les compilateurs modernes utilisent des instructions SIMD pour accelerer les operations vectorielles. Cette implementation necessite une gestion soignee de la memoire.",
            output: "isolated_test_ryan_fr.wav",
        },
        BenchSample {
            id: "en_zh",
            lang_code: "en",
            language: Language::English,
            text: "This pull request adds 3 new features and fixes 5 bugs. Please review 一下 the changes when you have time.",
            output: "isolated_test_ryan_en_zh.wav",
        },
        BenchSample {
            id: "zh_en",
            lang_code: "zh",
            language: Language::Chinese,
            text: "這次更新使用 React 和 Tauri 建立介面，核心是 Rust 撰寫，並整合 OCR 與 translation 流程。",
            output: "isolated_test_ryan_zh_en.wav",
        },
        BenchSample {
            id: "zh_v2",
            lang_code: "zh",
            language: Language::Chinese,
            text: "這是一段中文語音合成隔離測試，用來確認 qwen3-tts crate API 可以直接產生中文語音。",
            output: "isolated_test_ryan_zh_v2.wav",
        },
    ]
}

fn synthesize_sample(
    model: &Qwen3TTS,
    sample: &BenchSample,
    options: &SynthesisOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    let speaker_name = "Ryan";
    let preview: String = sample.text.chars().take(50).collect();

    println!();
    println!(
        "Synthesizing sample={} speaker={} language={} text_preview={:?}",
        sample.id, speaker_name, sample.lang_code, preview
    );
    println!("timestamp_before={:?}", SystemTime::now());

    let t0 = Instant::now();
    let audio = model.synthesize_with_voice(
        sample.text,
        Speaker::Ryan,
        sample.language,
        Some(options.clone()),
    )?;
    let elapsed_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let audio_duration_s = audio.duration() as f64;
    let rtf = if audio_duration_s > 0.0 {
        elapsed_ms / (audio_duration_s * 1000.0)
    } else {
        f64::INFINITY
    };

    println!("timestamp_after={:?}", SystemTime::now());
    println!(
        "sample={} speaker={} language={} elapsed_ms={elapsed_ms:.0} audio_duration_s={audio_duration_s:.2} rtf={rtf:.3} samples={}",
        sample.id,
        speaker_name,
        sample.lang_code,
        audio.len()
    );

    let out_path = std::env::current_dir()?.join(sample.output);
    audio.save(&out_path)?;
    println!("Saved: {}", out_path.display());
    println!(
        "RESULT|{}|{}|{}|{rtf:.6}|{elapsed_ms:.3}|{audio_duration_s:.6}|{}|{}",
        sample.id,
        speaker_name,
        sample.lang_code,
        audio.len(),
        out_path.display()
    );

    Ok(())
}

fn parse_result_line(stdout: &str) -> Option<BenchResult> {
    stdout.lines().find_map(|line| {
        let payload = line.strip_prefix("RESULT|")?;
        let parts = payload.split('|').collect::<Vec<_>>();
        if parts.len() != 8 {
            return None;
        }
        Some(BenchResult {
            id: parts[0].to_string(),
            speaker: parts[1].to_string(),
            lang_code: parts[2].to_string(),
            rtf: parts[3].parse().ok()?,
            elapsed_ms: parts[4].parse().ok()?,
            audio_duration_s: parts[5].parse().ok()?,
            samples: parts[6].parse().ok()?,
            output: parts[7].to_string(),
        })
    })
}

fn print_summary(results: &[BenchResult]) {
    println!();
    println!("=== Summary ===");
    println!("| Sample | Speaker | Lang | RTF | Synth ms | Audio s | Samples | WAV |");
    println!("| ------ | ------- | ---- | --- | -------- | ------- | ------- | --- |");
    for result in results {
        println!(
            "| {} | {} | {} | {:.3} | {:.0} | {:.2} | {} | {} |",
            result.id,
            result.speaker,
            result.lang_code,
            result.rtf,
            result.elapsed_ms,
            result.audio_duration_s,
            result.samples,
            result.output
        );
    }
}
