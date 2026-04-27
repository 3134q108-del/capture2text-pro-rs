use std::process::{Child, Command};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::app_dir;
use super::manifest::{self, ModelId};

static LLAMA_CHILD: OnceLock<Mutex<Option<Child>>> = OnceLock::new();

pub fn spawn_for(id: &ModelId) -> Result<(), String> {
    let spec = manifest::lookup(id).ok_or_else(|| "unknown model".to_string())?;
    let bin = app_dir().join("bin").join("llama-server.exe");
    let model = app_dir().join("models").join(spec.gguf_filename());
    let mmproj = app_dir().join("models").join(spec.mmproj_filename());

    if !bin.exists() {
        return Err(format!("missing llama-server binary: {}", bin.display()));
    }
    if !model.exists() {
        return Err(format!("missing model file: {}", model.display()));
    }
    if !mmproj.exists() {
        return Err(format!("missing mmproj file: {}", mmproj.display()));
    }

    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let mut command = Command::new(&bin);
    command
        .arg("--model")
        .arg(&model)
        .arg("--mmproj")
        .arg(&mmproj)
        .arg("--host")
        .arg("127.0.0.1")
        .arg("--port")
        .arg("11434")
        .arg("--n-gpu-layers")
        .arg("20")
        .arg("--ctx-size")
        .arg(spec.ctx_size.to_string())
        .arg("--chat-template")
        .arg(spec.chat_template)
        .creation_flags(CREATE_NO_WINDOW);

    let child = command
        .spawn()
        .map_err(|e| format!("spawn llama-server failed: {e}"))?;

    eprintln!("[llama-runtime] spawned pid={} for model={:?}", child.id(), id);
    let slot = LLAMA_CHILD.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = slot.lock() {
        *guard = Some(child);
    }

    poll_ready()
}

fn poll_ready() -> Result<(), String> {
    let started = Instant::now();
    let timeout = Duration::from_secs(120);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .map_err(|e| format!("health client build failed: {e}"))?;

    while started.elapsed() < timeout {
        std::thread::sleep(Duration::from_millis(500));
        if let Ok(response) = client.get("http://127.0.0.1:11434/health").send() {
            if response.status().is_success() {
                eprintln!(
                    "[llama-runtime] ready in {}ms",
                    started.elapsed().as_millis()
                );
                return Ok(());
            }
        }
    }

    Err("llama-server did not become ready within 120s".to_string())
}

pub fn stop() {
    if let Some(slot) = LLAMA_CHILD.get() {
        if let Ok(mut guard) = slot.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
                let _ = child.wait();
                eprintln!("[llama-runtime] stopped");
            }
        }
    }
}

pub fn is_healthy() -> bool {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .and_then(|client| client.get("http://127.0.0.1:11434/health").send())
        .map(|response| response.status().is_success())
        .unwrap_or(false)
}
