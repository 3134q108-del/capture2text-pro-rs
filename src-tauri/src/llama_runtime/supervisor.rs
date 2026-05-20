use std::process::{Child, Command};
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

use super::app_dir;
use super::manifest::{self, ModelId};

static LLAMA_CHILD: OnceLock<Mutex<Option<Child>>> = OnceLock::new();
static JOB_HANDLE: OnceLock<isize> = OnceLock::new();
static KEEPALIVE_STARTED: AtomicBool = AtomicBool::new(false);
const VISION_OFFLOAD_VRAM_THRESHOLD_GB: u64 = 16;
const BYTES_PER_GIB: u64 = 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OffloadMode {
    On,
    Off,
    Auto,
}

fn parse_offload_mode(env_val: Option<&str>) -> OffloadMode {
    match env_val.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("on") => OffloadMode::On,
        Some("off") => OffloadMode::Off,
        Some("auto") | None | Some("") => OffloadMode::Auto,
        Some(other) => {
            eprintln!(
                "[llama-runtime] invalid C2T_VISION_GPU_OFFLOAD={other:?}; fallback to auto"
            );
            OffloadMode::Auto
        }
    }
}

fn detect_max_vram_bytes() -> Option<u64> {
    unsafe {
        let factory: IDXGIFactory1 = CreateDXGIFactory1().ok()?;
        let mut max_vram = 0u64;
        let mut i = 0u32;
        loop {
            match factory.EnumAdapters(i) {
                Ok(adapter) => {
                    if let Ok(desc) = adapter.GetDesc() {
                        max_vram = max_vram.max(desc.DedicatedVideoMemory as u64);
                    }
                    i += 1;
                }
                Err(_) => break,
            }
        }
        if max_vram > 0 {
            Some(max_vram)
        } else {
            None
        }
    }
}

fn decide_disable_offload(env_val: Option<&str>, vram_bytes: Option<u64>) -> bool {
    match parse_offload_mode(env_val) {
        OffloadMode::On => false,
        OffloadMode::Off => true,
        OffloadMode::Auto => match vram_bytes {
            Some(bytes) => bytes < (VISION_OFFLOAD_VRAM_THRESHOLD_GB * BYTES_PER_GIB),
            None => true,
        },
    }
}

fn should_disable_gpu_offload() -> bool {
    let env_val = std::env::var("C2T_VISION_GPU_OFFLOAD").ok();
    let vram_bytes = detect_max_vram_bytes();
    let mode = parse_offload_mode(env_val.as_deref());
    let disable_offload = decide_disable_offload(env_val.as_deref(), vram_bytes);

    match mode {
        OffloadMode::On => {
            eprintln!("[llama-runtime] vision offload = GPU (env override)");
        }
        OffloadMode::Off => {
            eprintln!("[llama-runtime] vision offload = CPU (env override)");
        }
        OffloadMode::Auto => match vram_bytes {
            Some(bytes) => {
                let vram_gb = bytes / BYTES_PER_GIB;
                if disable_offload {
                    eprintln!(
                        "[llama-runtime] vision offload = CPU (auto: {vram_gb} GB VRAM < {VISION_OFFLOAD_VRAM_THRESHOLD_GB})"
                    );
                } else {
                    eprintln!(
                        "[llama-runtime] vision offload = GPU (auto: {vram_gb} GB VRAM >= {VISION_OFFLOAD_VRAM_THRESHOLD_GB})"
                    );
                }
            }
            None => {
                eprintln!(
                    "[llama-runtime] vision offload = CPU (auto: VRAM detect failed)"
                );
            }
        },
    }

    disable_offload
}

pub fn shared_async_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::ClientBuilder::new()
            .pool_idle_timeout(Some(Duration::from_secs(90)))
            .build()
            .expect("shared reqwest async client build failed")
    })
}

pub fn shared_blocking_client() -> &'static reqwest::blocking::Client {
    static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::blocking::ClientBuilder::new()
            .pool_idle_timeout(Some(Duration::from_secs(90)))
            .build()
            .expect("shared reqwest blocking client build failed")
    })
}

fn get_or_init_job() -> Option<HANDLE> {
    JOB_HANDLE.get_or_init(|| unsafe {
        let h = match CreateJobObjectW(None, None) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("[llama-runtime] CreateJobObject failed: {e:?}");
                return 0;
            }
        };

        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if let Err(e) = SetInformationJobObject(
            h,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const _,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        ) {
            eprintln!("[llama-runtime] SetInformationJobObject failed: {e:?}");
        }

        h.0 as isize
    });

    let raw = *JOB_HANDLE.get()?;
    if raw == 0 {
        None
    } else {
        Some(HANDLE(raw as *mut std::ffi::c_void))
    }
}

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

    let child = spawn_with_paths(&bin, &model, &mmproj, spec)?;
    store_child(child, id)?;
    poll_ready()?;
    start_keepalive();
    Ok(())
}

pub fn restart_with_model(model_id: ModelId) -> Result<(), String> {
    stop_current_server();

    let spec = model_id.spec();
    let models_dir = crate::app_paths::data_dir().join("models");
    let gguf = models_dir.join(spec.gguf_filename());
    let mmproj = models_dir.join(spec.mmproj_filename());
    let bin = app_dir().join("bin").join("llama-server.exe");

    if !bin.exists() {
        return Err(format!("missing llama-server binary: {}", bin.display()));
    }
    if !gguf.exists() || !mmproj.exists() {
        return Err(format!(
            "model files missing: {} or {}",
            gguf.display(),
            mmproj.display()
        ));
    }

    let child = spawn_with_paths(&bin, &gguf, &mmproj, spec)?;
    store_child(child, &model_id)?;
    poll_ready()?;
    start_keepalive();
    Ok(())
}

fn start_keepalive() {
    if KEEPALIVE_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }

    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            send_keepalive_ping().await;
        }
    });
}

async fn send_keepalive_ping() {
    let body = serde_json::json!({
        "model": "qwen3-vl",
        "messages": [{ "role": "user", "content": "hi" }],
        "max_tokens": 1,
        "stream": false,
    });
    let client = shared_async_client();
    let _ = client
        .post("http://127.0.0.1:11434/v1/chat/completions")
        .json(&body)
        .timeout(Duration::from_secs(5))
        .send()
        .await;
}

fn spawn_with_paths(
    bin: &Path,
    model: &Path,
    mmproj: &Path,
    spec: &manifest::ModelSpec,
) -> Result<Child, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let mut command = Command::new(bin);
    command
        .arg("--model")
        .arg(model)
        .arg("--mmproj")
        .arg(mmproj)
        .arg("--host")
        .arg("127.0.0.1")
        .arg("--port")
        .arg("11434")
        .arg("--n-gpu-layers")
        .arg("999");

    if should_disable_gpu_offload() {
        command.arg("--no-mmproj-offload");
    }

    command
        .arg("--ctx-size")
        .arg(spec.ctx_size.to_string())
        .arg("--batch-size")
        .arg("4096")
        .arg("--ubatch-size")
        .arg("2048")
        .arg("--flash-attn")
        .arg("off")
        .arg("--jinja")
        .arg("--cache-reuse")
        .arg("0")
        .arg("--cache-ram")
        .arg("0")
        .arg("--parallel")
        .arg("1")
        .creation_flags(CREATE_NO_WINDOW);

    command
        .spawn()
        .map_err(|e| format!("spawn llama-server failed: {e}"))
}

fn store_child(child: Child, id: &ModelId) -> Result<(), String> {
    unsafe {
        if let Some(job) = get_or_init_job() {
            let raw = child.as_raw_handle();
            let proc_handle = HANDLE(raw);
            if let Err(e) = AssignProcessToJobObject(job, proc_handle) {
                eprintln!(
                    "[llama-runtime] AssignProcessToJobObject failed (continuing without auto-cleanup): {e:?}"
                );
            } else {
                eprintln!("[llama-runtime] child assigned to job (auto-cleanup on parent death)");
            }
        }
    }

    eprintln!("[llama-runtime] spawned pid={} for model={:?}", child.id(), id);
    let slot = LLAMA_CHILD.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = slot.lock() {
        *guard = Some(child);
        Ok(())
    } else {
        Err("llama child lock poisoned".to_string())
    }
}

fn poll_ready() -> Result<(), String> {
    let started = Instant::now();
    let timeout = Duration::from_secs(300);
    let timeout_secs = timeout.as_secs();
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .map_err(|e| format!("health client build failed: {e}"))?;

    while started.elapsed() < timeout {
        std::thread::sleep(Duration::from_millis(500));
        if check_runtime_ready(&client) {
            eprintln!(
                "[llama-runtime] ready in {}ms",
                started.elapsed().as_millis()
            );
            return Ok(());
        }
    }

    Err(format!(
        "llama-server did not become ready within {timeout_secs}s"
    ))
}

pub fn stop() {
    stop_current_server();
}

fn stop_current_server() {
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
    let client = shared_blocking_client();
    check_runtime_ready(client)
}

fn check_runtime_ready(client: &reqwest::blocking::Client) -> bool {
    for attempt in 0..3 {
        if try_check_runtime_ready(client) {
            return true;
        }
        if attempt < 2 {
            std::thread::sleep(Duration::from_millis(500));
        }
    }
    false
}

fn try_check_runtime_ready(client: &reqwest::blocking::Client) -> bool {
    if let Ok(response) = client
        .get("http://127.0.0.1:11434/v1/models")
        .timeout(Duration::from_secs(5))
        .send()
    {
        if response.status().is_success() {
            return true;
        }
    }
    if let Ok(response) = client
        .get("http://127.0.0.1:11434/health")
        .timeout(Duration::from_secs(5))
        .send()
    {
        return response.status().is_success();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::decide_disable_offload;

    #[test]
    fn decide_disable_offload_on_forces_gpu() {
        assert!(!decide_disable_offload(Some("on"), Some(8 * 1024 * 1024 * 1024)));
    }

    #[test]
    fn decide_disable_offload_off_forces_cpu() {
        assert!(decide_disable_offload(
            Some("off"),
            Some(24 * 1024 * 1024 * 1024)
        ));
    }

    #[test]
    fn decide_disable_offload_none_none_defaults_safe_cpu() {
        assert!(decide_disable_offload(None, None));
    }
}
