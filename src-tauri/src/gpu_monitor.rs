use std::process::Command;
use std::thread;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::llama_runtime::manifest::ModelId;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct VramStatus {
    pub free: u64,
    pub total: u64,
    pub used: u64,
}

fn emit_or_log<T: Serialize>(app: &AppHandle, event: &str, payload: &T) {
    if let Err(err) = app.emit(event, payload) {
        eprintln!("[gpu-monitor] emit '{}' failed: {}", event, err);
    }
}

fn parse_u64_line(raw: &str) -> Option<u64> {
    raw.lines().next()?.trim().parse::<u64>().ok()
}

fn query_vram_values(args: &[&str]) -> Option<Vec<u64>> {
    let output = Command::new("nvidia-smi").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    let mut values = Vec::new();
    for value in stdout.lines().next()?.split(',') {
        values.push(value.trim().parse::<u64>().ok()?);
    }
    Some(values)
}

pub fn available_vram_mib() -> Option<u64> {
    let args = [
        "--query-gpu=memory.free",
        "--format=csv,noheader,nounits",
    ];
    query_vram_values(&args)
        .and_then(|mut values| values.pop())
        .or_else(|| {
            let output = Command::new("nvidia-smi").args(args).output().ok()?;
            if !output.status.success() {
                return None;
            }
            let stdout = String::from_utf8(output.stdout).ok()?;
            parse_u64_line(&stdout)
        })
}

pub fn required_vram_mib(model_id: &ModelId) -> u32 {
    model_id.spec().vram_mib
}

fn read_vram_status() -> Option<VramStatus> {
    let values = query_vram_values(&[
        "--query-gpu=memory.free,memory.total,memory.used",
        "--format=csv,noheader,nounits",
    ])?;
    if values.len() != 3 {
        return None;
    }
    Some(VramStatus {
        free: values[0],
        total: values[1],
        used: values[2],
    })
}

pub fn spawn_monitor(app_handle: AppHandle) {
    thread::spawn(move || loop {
        if let Some(status) = read_vram_status() {
            emit_or_log(&app_handle, "vlm-vram-status", &status);
        }
        thread::sleep(Duration::from_secs(10));
    });
}
