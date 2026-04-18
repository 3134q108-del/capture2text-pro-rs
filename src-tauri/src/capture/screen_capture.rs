use std::io;
use std::path::{Path, PathBuf};
use std::{env, fs};

use chrono::Local;
use image::RgbaImage;
use xcap::Monitor;

use crate::capture::{CursorPoint, HotkeyKind};

#[derive(Debug, Clone, Copy)]
pub struct CropRequest {
    pub crop_left_offset: i32,
    pub crop_top_offset: i32,
    pub crop_w: i32,
    pub crop_h: i32,
    pub pt_in_crop_x: i32,
    pub pt_in_crop_y: i32,
}

#[derive(Debug)]
pub struct CaptureOutput {
    pub monitor_x: i32,
    pub monitor_y: i32,
    pub monitor_w: i32,
    pub monitor_h: i32,
    pub image: RgbaImage,
    pub crop_x: i32,
    pub crop_y: i32,
    pub pt_x: i32,
    pub pt_y: i32,
}

pub fn capture_at_cursor(
    cursor: CursorPoint,
    request: CropRequest,
) -> io::Result<Option<CaptureOutput>> {
    let monitor = match Monitor::from_point(cursor.x, cursor.y) {
        Ok(monitor) => monitor,
        Err(_) => return Ok(None),
    };

    let monitor_x = monitor
        .x()
        .map_err(|err| io::Error::other(format!("monitor.x() failed: {err}")))?;
    let monitor_y = monitor
        .y()
        .map_err(|err| io::Error::other(format!("monitor.y() failed: {err}")))?;
    let monitor_w = monitor
        .width()
        .map_err(|err| io::Error::other(format!("monitor.width() failed: {err}")))?
        as i32;
    let monitor_h = monitor
        .height()
        .map_err(|err| io::Error::other(format!("monitor.height() failed: {err}")))?
        as i32;

    let local_x = cursor.x - monitor_x;
    let local_y = cursor.y - monitor_y;

    if local_x < 0 || local_y < 0 || local_x >= monitor_w || local_y >= monitor_h {
        return Ok(None);
    }

    let Some(crop) = build_clamped_crop(local_x, local_y, request, monitor_w, monitor_h) else {
        return Ok(None);
    };

    let image = monitor
        .capture_region(crop.crop_x as u32, crop.crop_y as u32, crop.crop_w as u32, crop.crop_h as u32)
        .map_err(|err| io::Error::other(format!("capture_region failed: {err}")))?;

    Ok(Some(CaptureOutput {
        monitor_x,
        monitor_y,
        monitor_w,
        monitor_h,
        image,
        crop_x: crop.crop_x,
        crop_y: crop.crop_y,
        pt_x: crop.pt_x,
        pt_y: crop.pt_y,
    }))
}

pub fn maybe_debug_save_capture(kind: HotkeyKind, image: &RgbaImage) {
    if !debug_save_enabled() {
        return;
    }

    if let Err(err) = save_capture_image(kind, image) {
        eprintln!("[capture] debug save failed: {err}");
    }
}

#[derive(Debug, Clone, Copy)]
struct ClampedCrop {
    crop_x: i32,
    crop_y: i32,
    crop_w: i32,
    crop_h: i32,
    pt_x: i32,
    pt_y: i32,
}

fn build_clamped_crop(
    local_x: i32,
    local_y: i32,
    request: CropRequest,
    monitor_w: i32,
    monitor_h: i32,
) -> Option<ClampedCrop> {
    let mut crop_x = local_x - request.crop_left_offset;
    let mut crop_y = local_y - request.crop_top_offset;
    let mut crop_w = request.crop_w;
    let mut crop_h = request.crop_h;
    let mut pt_x = request.pt_in_crop_x;
    let mut pt_y = request.pt_in_crop_y;

    if crop_x < 0 {
        let shift = -crop_x;
        crop_x = 0;
        crop_w -= shift;
        pt_x -= shift;
    }

    if crop_y < 0 {
        let shift = -crop_y;
        crop_y = 0;
        crop_h -= shift;
        pt_y -= shift;
    }

    if crop_x + crop_w > monitor_w {
        crop_w = monitor_w - crop_x;
    }

    if crop_y + crop_h > monitor_h {
        crop_h = monitor_h - crop_y;
    }

    if crop_w <= 0 || crop_h <= 0 {
        return None;
    }

    if pt_x < 0 || pt_y < 0 || pt_x >= crop_w || pt_y >= crop_h {
        return None;
    }

    Some(ClampedCrop {
        crop_x,
        crop_y,
        crop_w,
        crop_h,
        pt_x,
        pt_y,
    })
}

fn debug_save_enabled() -> bool {
    matches!(env::var("C2T_DEBUG_SAVE").ok().as_deref(), Some("1"))
}

fn save_capture_image(kind: HotkeyKind, image: &RgbaImage) -> io::Result<PathBuf> {
    let capture_dir = capture_directory()?;
    fs::create_dir_all(&capture_dir)?;

    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let base_name = format!("{}_{}", timestamp, kind.as_suffix());
    let output_path = next_available_png_path(&capture_dir, &base_name)?;

    image
        .save(&output_path)
        .map_err(|err| io::Error::other(format!("failed to save png: {err}")))?;

    Ok(output_path)
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
