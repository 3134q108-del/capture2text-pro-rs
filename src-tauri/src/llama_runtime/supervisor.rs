use std::fs::{self, OpenOptions};
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

use super::manifest::{self, ModelId};
use super::{app_dir, SWITCH_LOCK};

pub const LLAMA_PORT: u16 = 11500;

static LLAMA_CHILD: OnceLock<Mutex<Option<Child>>> = OnceLock::new();
static JOB_HANDLE: OnceLock<isize> = OnceLock::new();
static KEEPALIVE_STARTED: AtomicBool = AtomicBool::new(false);
static WATCHDOG_STARTED: AtomicBool = AtomicBool::new(false);
static INFERENCE_COUNT: AtomicU64 = AtomicU64::new(0);
static RESTART_COUNT: AtomicU64 = AtomicU64::new(0);
static CRASH_RESTART_COUNT: AtomicU64 = AtomicU64::new(0);
static SERVER_GENERATION: AtomicU64 = AtomicU64::new(0);
static CURRENT_MODEL: OnceLock<Mutex<Option<ModelId>>> = OnceLock::new();
static RESTART_IN_PROGRESS: AtomicBool = AtomicBool::new(false);
static EXPECTED_STOP: AtomicBool = AtomicBool::new(false);
static CRASH_RESTART_STATE: OnceLock<Mutex<CrashRestartState>> = OnceLock::new();
const VISION_OFFLOAD_VRAM_THRESHOLD_GB: u64 = 16;
const BYTES_PER_GIB: u64 = 1024 * 1024 * 1024;
const LLAMA_SERVER_LOG_ROTATE_BYTES: u64 = 5 * 1024 * 1024;
const LLAMA_SERVER_LOG_WAIT_TIMEOUT: Duration = Duration::from_secs(330);

#[derive(Default)]
struct CrashRestartState {
    consecutive_crashes: u32,
    last_crash_at: Option<Instant>,
    auto_restart_disabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnsureRunningDecision {
    Healthy,
    WaitForRestart,
    NoCurrentModel,
    Restart(ModelId),
}

struct RestartInProgressGuard;

impl Drop for RestartInProgressGuard {
    fn drop(&mut self) {
        RESTART_IN_PROGRESS.store(false, Ordering::SeqCst);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OffloadMode {
    On,
    Off,
    Auto,
}

fn parse_offload_mode(env_val: Option<&str>) -> OffloadMode {
    match env_val
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("on") => OffloadMode::On,
        Some("off") => OffloadMode::Off,
        Some("auto") | None | Some("") => OffloadMode::Auto,
        Some(other) => {
            eprintln!("[llama-runtime] invalid C2T_VISION_GPU_OFFLOAD={other:?}; fallback to auto");
            OffloadMode::Auto
        }
    }
}

fn detect_max_vram_bytes() -> Option<u64> {
    unsafe {
        let factory: IDXGIFactory1 = CreateDXGIFactory1().ok()?;
        let mut max_vram = 0u64;
        let mut i = 0u32;
        while let Ok(adapter) = factory.EnumAdapters(i) {
            if let Ok(desc) = adapter.GetDesc() {
                max_vram = max_vram.max(desc.DedicatedVideoMemory as u64);
            }
            i += 1;
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
                eprintln!("[llama-runtime] vision offload = CPU (auto: VRAM detect failed)");
            }
        },
    }

    disable_offload
}

pub fn shared_async_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::ClientBuilder::new()
            .pool_max_idle_per_host(0)
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

pub fn record_inference_done() {
    let count = INFERENCE_COUNT.fetch_add(1, Ordering::SeqCst) + 1;
    reset_crash_restart_state();
    if should_trigger_restart(count) {
        spawn_restart_task();
    }
}

pub fn inference_count() -> u64 {
    INFERENCE_COUNT.load(Ordering::SeqCst)
}

pub fn restart_count() -> u64 {
    RESTART_COUNT.load(Ordering::SeqCst)
}

fn decide_should_restart(count: u64, threshold: u64, in_progress: bool) -> bool {
    threshold > 0 && count >= threshold && !in_progress
}

fn restart_threshold() -> u64 {
    std::env::var("C2T_LLAMA_RESTART_THRESHOLD")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(500)
}

fn should_trigger_restart(count: u64) -> bool {
    let threshold = restart_threshold();
    let in_progress = RESTART_IN_PROGRESS.load(Ordering::SeqCst);
    decide_should_restart(count, threshold, in_progress)
}

fn decide_ensure_running_action(
    healthy: bool,
    restart_in_progress: bool,
    current_model: Option<ModelId>,
) -> EnsureRunningDecision {
    if healthy {
        EnsureRunningDecision::Healthy
    } else if restart_in_progress {
        EnsureRunningDecision::WaitForRestart
    } else if let Some(model_id) = current_model {
        EnsureRunningDecision::Restart(model_id)
    } else {
        EnsureRunningDecision::NoCurrentModel
    }
}

pub fn ensure_running() -> Result<(), String> {
    let healthy = is_healthy();
    let restart_in_progress = RESTART_IN_PROGRESS.load(Ordering::SeqCst);
    let tracked_model = current_model();

    match decide_ensure_running_action(healthy, restart_in_progress, tracked_model) {
        EnsureRunningDecision::Healthy | EnsureRunningDecision::NoCurrentModel => Ok(()),
        EnsureRunningDecision::WaitForRestart => {
            wait_for_runtime_healthy(LLAMA_SERVER_LOG_WAIT_TIMEOUT)
        }
        EnsureRunningDecision::Restart(_) => {
            if RESTART_IN_PROGRESS
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
            {
                return wait_for_runtime_healthy(LLAMA_SERVER_LOG_WAIT_TIMEOUT);
            }

            let _restart_guard = RestartInProgressGuard;
            let _switch_guard = SWITCH_LOCK
                .lock()
                .map_err(|e| format!("switch_lock poisoned: {e}"))?;

            if is_healthy() {
                return Ok(());
            }

            let Some(model_id) = current_model() else {
                return Ok(());
            };

            restart_with_model(model_id)
        }
    }
}

fn spawn_restart_task() {
    if RESTART_IN_PROGRESS.swap(true, Ordering::SeqCst) {
        return;
    }
    RESTART_COUNT.fetch_add(1, Ordering::SeqCst);

    std::thread::spawn(|| {
        eprintln!(
            "[llama-restart] threshold reached ({} inferences), restarting llama-server",
            INFERENCE_COUNT.load(Ordering::SeqCst)
        );

        let model_id = CURRENT_MODEL
            .get()
            .and_then(|slot| slot.lock().ok())
            .and_then(|guard| *guard);

        if let Some(id) = model_id {
            EXPECTED_STOP.store(true, Ordering::SeqCst);
            match restart_with_model(id) {
                Ok(()) => {
                    INFERENCE_COUNT.store(0, Ordering::SeqCst);
                    eprintln!("[llama-restart] complete, counter reset");
                }
                Err(e) => eprintln!("[llama-restart] failed: {e}"),
            }
        } else {
            eprintln!("[llama-restart] skipped: no current model tracked");
        }

        RESTART_IN_PROGRESS.store(false, Ordering::SeqCst);
    });
}

fn crash_restart_state() -> &'static Mutex<CrashRestartState> {
    CRASH_RESTART_STATE.get_or_init(|| Mutex::new(CrashRestartState::default()))
}

fn reset_crash_restart_state() {
    if let Ok(mut guard) = crash_restart_state().lock() {
        guard.consecutive_crashes = 0;
        guard.last_crash_at = None;
        guard.auto_restart_disabled = false;
    }
}

fn crash_restart_delay(consecutive_crashes: u32) -> Option<Duration> {
    match consecutive_crashes {
        0..=2 => Some(Duration::from_secs(0)),
        3..=5 => {
            let seconds = 2_u64.pow(consecutive_crashes);
            Some(Duration::from_secs(seconds.min(60)))
        }
        _ => None,
    }
}

fn start_watchdog() {
    if WATCHDOG_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }

    std::thread::spawn(|| loop {
        std::thread::sleep(Duration::from_secs(2));
        if RESTART_IN_PROGRESS.load(Ordering::SeqCst) {
            continue;
        }

        let Some(slot) = LLAMA_CHILD.get() else {
            continue;
        };

        let Ok(mut guard) = slot.try_lock() else {
            continue;
        };

        let Some(child) = guard.as_mut() else {
            continue;
        };

        let status = match child.try_wait() {
            Ok(Some(status)) => status,
            Ok(None) => continue,
            Err(err) => {
                eprintln!("[llama-watchdog] try_wait failed: {err}");
                continue;
            }
        };

        let expected_stop = EXPECTED_STOP.swap(false, Ordering::SeqCst);
        let model_id = current_model();
        guard.take();
        drop(guard);

        if expected_stop {
            continue;
        }

        let Some(model_id) = model_id else {
            eprintln!("[llama-watchdog] child exited unexpectedly: status={status}; no current model tracked");
            continue;
        };

        let (delay, consecutive_crashes, newly_disabled) = {
            let mut state = match crash_restart_state().lock() {
                Ok(guard) => guard,
                Err(err) => {
                    eprintln!("[llama-watchdog] crash state lock poisoned: {err}");
                    continue;
                }
            };

            if let Some(last_crash_at) = state.last_crash_at {
                if last_crash_at.elapsed() > Duration::from_secs(120) {
                    state.consecutive_crashes = 0;
                }
            }

            let consecutive_crashes = state.consecutive_crashes;
            let delay = crash_restart_delay(consecutive_crashes);
            state.consecutive_crashes = consecutive_crashes.saturating_add(1);
            state.last_crash_at = Some(Instant::now());
            let newly_disabled = delay.is_none() && !state.auto_restart_disabled;
            if delay.is_none() {
                state.auto_restart_disabled = true;
            }
            (delay, consecutive_crashes, newly_disabled)
        };

        eprintln!(
                "[llama-watchdog] child exited unexpectedly: status={status}, consecutive_crashes={}, restart_count={}",
                consecutive_crashes,
                CRASH_RESTART_COUNT.load(Ordering::SeqCst) + 1
            );

        let Some(delay) = delay else {
            if newly_disabled {
                eprintln!(
                        "[llama-watchdog] crash loop、停止自動重生: status={status}, consecutive_crashes={}",
                        consecutive_crashes
                    );
            }
            continue;
        };

        if RESTART_IN_PROGRESS.swap(true, Ordering::SeqCst) {
            continue;
        }

        std::thread::spawn(move || {
            if !delay.is_zero() {
                std::thread::sleep(delay);
            }

            if current_model() != Some(model_id) {
                eprintln!(
                        "[llama-watchdog] crash restart skipped: model changed or stopped before restart"
                    );
                RESTART_IN_PROGRESS.store(false, Ordering::SeqCst);
                return;
            }

            let _switch_guard = match SWITCH_LOCK.try_lock() {
                Ok(guard) => guard,
                Err(_) => {
                    eprintln!("[llama-watchdog] crash restart skipped: switch in progress");
                    RESTART_IN_PROGRESS.store(false, Ordering::SeqCst);
                    return;
                }
            };

            if current_model() != Some(model_id) {
                eprintln!(
                        "[llama-watchdog] crash restart skipped: model changed or stopped after switch lock"
                    );
                RESTART_IN_PROGRESS.store(false, Ordering::SeqCst);
                return;
            }

            CRASH_RESTART_COUNT.fetch_add(1, Ordering::SeqCst);
            let restart_result = restart_with_model_internal(model_id, false);
            if let Err(err) = restart_result {
                eprintln!("[llama-watchdog] crash restart failed: {err}");
            }

            RESTART_IN_PROGRESS.store(false, Ordering::SeqCst);
        });
    });
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
    reset_crash_restart_state();
    finish_spawn()?;
    Ok(())
}

pub fn restart_with_model(model_id: ModelId) -> Result<(), String> {
    restart_with_model_internal(model_id, true)
}

pub fn server_generation() -> u64 {
    SERVER_GENERATION.load(Ordering::SeqCst)
}

fn restart_with_model_internal(model_id: ModelId, reset_crash_state: bool) -> Result<(), String> {
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
    if reset_crash_state {
        reset_crash_restart_state();
    }
    finish_spawn()?;
    Ok(())
}

fn finish_spawn() -> Result<(), String> {
    poll_ready()?;
    start_keepalive();
    SERVER_GENERATION.fetch_add(1, Ordering::SeqCst);
    crate::vlm::warmup();
    Ok(())
}

fn wait_for_runtime_healthy(timeout: Duration) -> Result<(), String> {
    let started = Instant::now();
    let timeout_secs = timeout.as_secs();
    while started.elapsed() < timeout {
        if is_healthy() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    Err(format!(
        "llama-server did not become healthy within {timeout_secs}s"
    ))
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
        .post(format!("http://127.0.0.1:{LLAMA_PORT}/v1/chat/completions"))
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
        .arg(LLAMA_PORT.to_string())
        .arg("--log-timestamps")
        .arg("--n-gpu-layers")
        .arg("999");

    if should_disable_gpu_offload() {
        command.arg("--no-mmproj-offload");
    }

    command
        .arg("--ctx-size")
        .arg(spec.ctx_size.to_string())
        .arg("--batch-size")
        .arg("1024")
        .arg("--ubatch-size")
        .arg("512")
        .arg("--flash-attn")
        .arg("auto")
        .arg("--jinja")
        .arg("--cache-reuse")
        .arg("0")
        .arg("--cache-ram")
        .arg("0")
        .arg("--parallel")
        .arg("1")
        .creation_flags(CREATE_NO_WINDOW);

    if let Some((stdout, stderr)) = llama_server_stdio() {
        command.stdout(stdout).stderr(stderr);
    }

    command
        .spawn()
        .map_err(|e| format!("spawn llama-server failed: {e}"))
}

fn llama_server_stdio() -> Option<(Stdio, Stdio)> {
    let log_dir = app_dir().join("logs");
    if let Err(err) = fs::create_dir_all(&log_dir) {
        eprintln!(
            "[llama-runtime] create log dir {} failed: {err}",
            log_dir.display()
        );
        return None;
    }

    let log_path = log_dir.join("llama-server.log");
    if let Ok(metadata) = fs::metadata(&log_path) {
        if metadata.len() > LLAMA_SERVER_LOG_ROTATE_BYTES {
            let rotated_path = log_dir.join("llama-server.log.1");
            if let Err(err) = fs::rename(&log_path, &rotated_path) {
                eprintln!(
                    "[llama-runtime] rotate log {} -> {} failed: {err}; truncating",
                    log_path.display(),
                    rotated_path.display()
                );
                if let Err(err) = OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(&log_path)
                {
                    eprintln!(
                        "[llama-runtime] truncate log {} failed: {err}",
                        log_path.display()
                    );
                    return None;
                }
            }
        }
    }

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .ok()?;
    let stderr = file.try_clone().ok()?;
    Some((Stdio::from(file), Stdio::from(stderr)))
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

    eprintln!(
        "[llama-runtime] spawned pid={} for model={:?}",
        child.id(),
        id
    );
    let slot = LLAMA_CHILD.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = slot.lock() {
        *guard = Some(child);
        set_current_model(Some(*id));
        start_watchdog();
        Ok(())
    } else {
        Err("llama child lock poisoned".to_string())
    }
}

fn set_current_model(id: Option<ModelId>) {
    let slot = CURRENT_MODEL.get_or_init(|| Mutex::new(None));
    match slot.lock() {
        Ok(mut guard) => {
            *guard = id;
        }
        Err(_) => {
            eprintln!("[llama-runtime] current model lock poisoned");
        }
    }
}

fn current_model() -> Option<ModelId> {
    let slot = CURRENT_MODEL.get_or_init(|| Mutex::new(None));
    slot.lock().ok().and_then(|guard| *guard)
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
    EXPECTED_STOP.store(true, Ordering::SeqCst);
    set_current_model(None);
    if let Some(slot) = LLAMA_CHILD.get() {
        if let Ok(mut guard) = slot.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
                let _ = child.wait();
                eprintln!("[llama-runtime] stopped");
            }
        }
    }
    EXPECTED_STOP.store(false, Ordering::SeqCst);
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
        .get(format!("http://127.0.0.1:{LLAMA_PORT}/v1/models"))
        .timeout(Duration::from_secs(5))
        .send()
    {
        if response.status().is_success() {
            return true;
        }
    }
    if let Ok(response) = client
        .get(format!("http://127.0.0.1:{LLAMA_PORT}/health"))
        .timeout(Duration::from_secs(5))
        .send()
    {
        return response.status().is_success();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{
        crash_restart_delay, decide_disable_offload, decide_ensure_running_action,
        decide_should_restart, EnsureRunningDecision, ModelId,
    };
    use std::time::Duration;

    #[test]
    fn decide_disable_offload_on_forces_gpu() {
        assert!(!decide_disable_offload(
            Some("on"),
            Some(8 * 1024 * 1024 * 1024)
        ));
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

    #[test]
    fn decide_should_restart_threshold_zero_disables_restart() {
        assert!(!decide_should_restart(500, 0, false));
    }

    #[test]
    fn decide_should_restart_count_below_threshold_does_not_restart() {
        assert!(!decide_should_restart(499, 500, false));
    }

    #[test]
    fn decide_should_restart_count_at_threshold_restarts() {
        assert!(decide_should_restart(500, 500, false));
    }

    #[test]
    fn decide_should_restart_in_progress_does_not_restart() {
        assert!(!decide_should_restart(500, 500, true));
    }

    #[test]
    fn decide_ensure_running_healthy_is_noop() {
        assert_eq!(
            decide_ensure_running_action(true, false, Some(ModelId::Qwen3Vl2bInstruct)),
            EnsureRunningDecision::Healthy
        );
    }

    #[test]
    fn decide_ensure_running_no_current_model_is_noop() {
        assert_eq!(
            decide_ensure_running_action(false, false, None),
            EnsureRunningDecision::NoCurrentModel
        );
    }

    #[test]
    fn decide_ensure_running_restart_in_progress_waits() {
        assert_eq!(
            decide_ensure_running_action(false, true, None),
            EnsureRunningDecision::WaitForRestart
        );
    }

    #[test]
    fn decide_ensure_running_unhealthy_with_model_restarts() {
        assert_eq!(
            decide_ensure_running_action(false, false, Some(ModelId::Qwen3Vl4bInstruct)),
            EnsureRunningDecision::Restart(ModelId::Qwen3Vl4bInstruct)
        );
    }

    #[test]
    fn crash_restart_delay_is_immediate_for_first_three_crashes() {
        assert_eq!(crash_restart_delay(0), Some(Duration::from_secs(0)));
        assert_eq!(crash_restart_delay(1), Some(Duration::from_secs(0)));
        assert_eq!(crash_restart_delay(2), Some(Duration::from_secs(0)));
    }

    #[test]
    fn crash_restart_delay_exponentially_backs_off_then_caps() {
        assert_eq!(crash_restart_delay(3), Some(Duration::from_secs(8)));
        assert_eq!(crash_restart_delay(4), Some(Duration::from_secs(16)));
        assert_eq!(crash_restart_delay(5), Some(Duration::from_secs(32)));
    }

    #[test]
    fn crash_restart_delay_stops_after_crash_loop() {
        assert_eq!(crash_restart_delay(6), None);
        assert_eq!(crash_restart_delay(42), None);
    }
}
