use std::fs::{self, File};
use std::io::copy;
use std::time::Duration;

const CUSTOMVOICE_REPO: &str = "Qwen/Qwen3-TTS-12Hz-0.6B-CustomVoice";
const SPEECH_TOKENIZER_REPO: &str = "Qwen/Qwen3-TTS-Tokenizer-12Hz";
const TEXT_TOKENIZER_REPO: &str = "Qwen/Qwen2-0.5B";

type RequiredFile = (&'static str, &'static str, &'static str);

const REQUIRED_FILES: &[RequiredFile] = &[
    (CUSTOMVOICE_REPO, "config.json", "config.json"),
    (CUSTOMVOICE_REPO, "model.safetensors", "model.safetensors"),
    (
        SPEECH_TOKENIZER_REPO,
        "model.safetensors",
        "speech_tokenizer/model.safetensors",
    ),
    (TEXT_TOKENIZER_REPO, "tokenizer.json", "tokenizer.json"),
];

pub fn ensure_customvoice_installed() -> Result<(), String> {
    let target_dir = super::customvoice_model_dir();
    if customvoice_ready(&target_dir) {
        return Ok(());
    }

    fs::create_dir_all(&target_dir).map_err(|e| {
        format!(
            "create customvoice model dir failed ({}): {e}",
            target_dir.display()
        )
    })?;

    eprintln!(
        "[qwen-tts] downloading model files into {}",
        target_dir.display()
    );

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(1800))
        .build()
        .map_err(|e| e.to_string())?;

    for &(repo, remote, local) in REQUIRED_FILES {
        download_one(&client, &target_dir, repo, remote, local)?;
    }

    if !customvoice_ready(&target_dir) {
        return Err(format!(
            "model download incomplete in {}",
            target_dir.display()
        ));
    }
    Ok(())
}

fn download_one(
    client: &reqwest::blocking::Client,
    target_dir: &std::path::Path,
    repo: &str,
    remote_name: &str,
    local_relative_path: &str,
) -> Result<(), String> {
    let dst = target_dir.join(local_relative_path);
    if is_present(&dst) {
        return Ok(());
    }

    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let url = format!(
        "https://huggingface.co/{repo}/resolve/main/{remote_name}?download=true"
    );
    eprintln!("[qwen-tts] downloading {repo}/{remote_name} ...");

    let mut resp = client
        .get(&url)
        .header("User-Agent", "capture2text-pro-rs")
        .send()
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!(
            "download {repo}/{remote_name} failed with HTTP {}",
            resp.status()
        ));
    }

    let mut out = File::create(&dst).map_err(|e| format!("create {} failed: {e}", dst.display()))?;
    copy(&mut resp, &mut out).map_err(|e| format!("write {} failed: {e}", dst.display()))?;
    Ok(())
}

fn customvoice_ready(dir: &std::path::Path) -> bool {
    REQUIRED_FILES
        .iter()
        .all(|(_, _, local)| is_present(&dir.join(local)))
}

fn is_present(path: &std::path::Path) -> bool {
    fs::metadata(path)
        .map(|meta| meta.is_file() && meta.len() > 0)
        .unwrap_or(false)
}
