use std::process::{Child, Command};
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

use super::app_dir;
use super::manifest::{self, ModelId};

static LLAMA_CHILD: OnceLock<Mutex<Option<Child>>> = OnceLock::new();
static JOB_HANDLE: OnceLock<isize> = OnceLock::new();
static KEEPALIVE_STARTED: AtomicBool = AtomicBool::new(false);

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
    let client = reqwest::Client::new();
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
        .arg("999")
        .arg("--ctx-size")
        .arg(spec.ctx_size.to_string())
        .arg("--batch-size")
        .arg("4096")
        .arg("--ubatch-size")
        .arg("2048")
        .arg("--flash-attn")
        .arg("on")
        .creation_flags(CREATE_NO_WINDOW);

    command
        .spawn()
        .map_err(|e| format!("spawn llama-server failed: {e}"))
}

fn store_child(child: Child, id: &ModelId) -> Result<(), String> {
    unsafe {
        if let Some(job) = get_or_init_job() {
            let raw = child.as_raw_handle();
            let proc_handle = HANDLE(raw as *mut std::ffi::c_void);
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
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(client) => client,
        Err(_) => return false,
    };
    check_runtime_ready(&client)
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
    if let Ok(response) = client.get("http://127.0.0.1:11434/v1/models").send() {
        if response.status().is_success() {
            return true;
        }
    }
    if let Ok(response) = client.get("http://127.0.0.1:11434/health").send() {
        return response.status().is_success();
    }
    false
}
