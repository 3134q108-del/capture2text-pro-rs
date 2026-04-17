use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

use chrono::Local;
use image::{ImageFormat, RgbaImage};
use xcap::Monitor;

use crate::capture::HotkeyKind;

pub(crate) fn worker_loop(rx: Receiver<HotkeyKind>) {
    for kind in rx {
        let _ = capture_and_save(kind);
    }
}

fn capture_and_save(kind: HotkeyKind) -> io::Result<PathBuf> {
    let image = capture_primary_monitor()?;
    let capture_dir = capture_directory()?;
    fs::create_dir_all(&capture_dir)?;

    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let base_name = format!("{}_{}", timestamp, kind.as_suffix());
    let output_path = next_available_png_path(&capture_dir, &base_name)?;

    image
        .save_with_format(&output_path, ImageFormat::Png)
        .map_err(|err| io::Error::other(format!("failed to save png: {err}")))?;

    Ok(output_path)
}

fn capture_primary_monitor() -> io::Result<RgbaImage> {
    let mut monitors = Monitor::all().map_err(|err| io::Error::other(format!("list monitors failed: {err}")))?;
    if monitors.is_empty() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "no monitors available"));
    }

    let primary_index = monitors
        .iter()
        .position(|monitor| monitor.is_primary().unwrap_or(false))
        .unwrap_or(0);
    let monitor = monitors.swap_remove(primary_index);

    monitor
        .capture_image()
        .map_err(|err| io::Error::other(format!("capture monitor failed: {err}")))
}

fn capture_directory() -> io::Result<PathBuf> {
    let local_data_dir = dirs::data_local_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "local app data directory not found"))?;

    Ok(local_data_dir.join("Capture2TextPro").join("captures"))
}

fn next_available_png_path(capture_dir: &Path, base_name: &str) -> io::Result<PathBuf> {
    let initial_path = capture_dir.join(format!("{base_name}.png"));
    if !initial_path.exists() {
        return Ok(initial_path);
    }

    for index in 1..=999 {
        let candidate = capture_dir.join(format!("{base_name}_{index:03}.png"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "too many captures in the same second for this hotkey",
    ))
}
