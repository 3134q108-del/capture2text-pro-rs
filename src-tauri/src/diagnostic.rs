use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::Manager;
use windows::core::Interface;
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIAdapter3, IDXGIFactory1, DXGI_MEMORY_SEGMENT_GROUP_LOCAL,
    DXGI_QUERY_VIDEO_MEMORY_INFO,
};
use windows::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows::Win32::System::Threading::GetCurrentProcess;

static DIAG_WORKER_STARTED: AtomicBool = AtomicBool::new(false);

const DEFAULT_INTERVAL_SECS: u64 = 60;
const MIN_INTERVAL_SECS: u64 = 5;
const MAX_INTERVAL_SECS: u64 = 3600;
const BYTES_PER_MIB: u64 = 1024 * 1024;

#[derive(Debug, Serialize)]
pub struct DiagSnapshot {
    pub timestamp_epoch: u64,
    pub gpu_vram_used_mb: Option<u64>,
    pub gpu_vram_budget_mb: Option<u64>,
    pub gdi_objects: Option<u32>,
    pub user_objects: Option<u32>,
    pub working_set_mb: u64,
    pub inference_count: u64,
    pub restart_count: u64,
    pub uptime_secs: u64,
}

pub fn start_worker(app_handle: tauri::AppHandle) {
    if diagnostic_disabled() {
        eprintln!("[diagnostic] disabled by C2T_DIAG_DISABLED=1");
        return;
    }
    if DIAG_WORKER_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }

    let interval = Duration::from_secs(diagnostic_interval_secs());
    let started_at = Instant::now();
    if let Err(err) = std::thread::Builder::new()
        .name("diagnostic-worker".to_string())
        .spawn(move || {
            loop {
                std::thread::sleep(interval);
                let snapshot = collect_snapshot(started_at);
                append_snapshot(&app_handle, &snapshot);
            }
        })
    {
        DIAG_WORKER_STARTED.store(false, Ordering::SeqCst);
        eprintln!("[diagnostic] worker spawn failed: {err}");
    }
}

fn collect_snapshot(started_at: Instant) -> DiagSnapshot {
    let (gpu_vram_used_mb, gpu_vram_budget_mb) = read_gpu_vram_mb();
    DiagSnapshot {
        timestamp_epoch: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0),
        gpu_vram_used_mb,
        gpu_vram_budget_mb,
        gdi_objects: read_gui_resources(GR_GDIOBJECTS),
        user_objects: read_gui_resources(GR_USEROBJECTS),
        working_set_mb: read_working_set_mb(),
        inference_count: crate::llama_runtime::supervisor::inference_count(),
        restart_count: crate::llama_runtime::supervisor::restart_count(),
        uptime_secs: started_at.elapsed().as_secs(),
    }
}

fn append_snapshot(app_handle: &tauri::AppHandle, snapshot: &DiagSnapshot) {
    let dir = match app_handle.path().app_log_dir() {
        Ok(dir) => dir,
        Err(err) => {
            eprintln!("[diagnostic] app_log_dir failed: {err}");
            return;
        }
    };
    if let Err(err) = fs::create_dir_all(&dir) {
        eprintln!("[diagnostic] create log dir {} failed: {err}", dir.display());
        return;
    }

    let path = dir.join("diagnostic.log");
    let line = match serde_json::to_string(snapshot) {
        Ok(line) => line,
        Err(err) => {
            eprintln!("[diagnostic] serialize snapshot failed: {err}");
            return;
        }
    };

    match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(mut file) => {
            if let Err(err) = writeln!(file, "{line}") {
                eprintln!("[diagnostic] write {} failed: {err}", path.display());
            }
        }
        Err(err) => eprintln!("[diagnostic] open {} failed: {err}", path.display()),
    }
}

fn diagnostic_disabled() -> bool {
    std::env::var("C2T_DIAG_DISABLED")
        .map(|value| value.trim() == "1")
        .unwrap_or(false)
}

fn diagnostic_interval_secs() -> u64 {
    clamp_interval_secs(
        std::env::var("C2T_DIAG_INTERVAL")
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok())
            .unwrap_or(DEFAULT_INTERVAL_SECS),
    )
}

fn clamp_interval_secs(value: u64) -> u64 {
    value.clamp(MIN_INTERVAL_SECS, MAX_INTERVAL_SECS)
}

fn read_gpu_vram_mb() -> (Option<u64>, Option<u64>) {
    unsafe {
        let factory: IDXGIFactory1 = match CreateDXGIFactory1() {
            Ok(factory) => factory,
            Err(_) => return (None, None),
        };
        let adapter = match factory.EnumAdapters(0) {
            Ok(adapter) => adapter,
            Err(_) => return (None, None),
        };
        let adapter3: IDXGIAdapter3 = match adapter.cast() {
            Ok(adapter) => adapter,
            Err(_) => return (None, None),
        };
        let mut info = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
        if adapter3
            .QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, &mut info)
            .is_err()
        {
            return (None, None);
        }
        (
            Some(info.CurrentUsage / BYTES_PER_MIB),
            Some(info.Budget / BYTES_PER_MIB),
        )
    }
}

const GR_GDIOBJECTS: u32 = 0;
const GR_USEROBJECTS: u32 = 1;

fn read_gui_resources(flag: u32) -> Option<u32> {
    let value = unsafe { get_gui_resources(GetCurrentProcess(), flag) };
    if value == 0 {
        None
    } else {
        Some(value)
    }
}

#[link(name = "user32")]
extern "system" {
    #[link_name = "GetGuiResources"]
    fn get_gui_resources(process: windows::Win32::Foundation::HANDLE, flags: u32) -> u32;
}

fn read_working_set_mb() -> u64 {
    unsafe {
        let mut counters = PROCESS_MEMORY_COUNTERS {
            cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            ..Default::default()
        };
        if K32GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut counters,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        )
        .as_bool()
        {
            counters.WorkingSetSize as u64 / BYTES_PER_MIB
        } else {
            0
        }
    }
}
