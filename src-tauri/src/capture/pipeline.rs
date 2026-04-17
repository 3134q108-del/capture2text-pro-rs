use std::io;

use image::codecs::png::PngEncoder;
use image::imageops;
use image::{ColorType, ImageEncoder, RgbaImage};
use xcap::Monitor;

use crate::capture::params::{profile_for, ModeProfile};
use crate::capture::{HotkeyEvent, HotkeyKind};
use crate::capture::preprocess::{extract_text_block, ExtractParams, OCR_SCALE_FACTOR_DEFAULT};
use crate::leptonica::Pix;

#[derive(Debug, Clone, Copy)]
pub struct BoundingBoxScreen {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

pub fn run_for_event(event: HotkeyEvent) -> io::Result<Option<BoundingBoxScreen>> {
    let Some(profile) = profile_for(event.kind) else {
        return Ok(None);
    };

    let capture = capture_primary_monitor()?;
    let local_x = event.cursor.x - capture.monitor_x;
    let local_y = event.cursor.y - capture.monitor_y;

    if local_x < 0
        || local_y < 0
        || local_x >= capture.monitor_w
        || local_y >= capture.monitor_h
    {
        return Ok(None);
    }

    let Some(crop) = build_clamped_crop(local_x, local_y, profile, capture.monitor_w, capture.monitor_h) else {
        return Ok(None);
    };

    let cropped = imageops::crop_imm(
        &capture.image,
        crop.x as u32,
        crop.y as u32,
        crop.w as u32,
        crop.h as u32,
    )
    .to_image();

    let png_bytes = encode_png(&cropped)?;
    let pix_crop = Pix::from_bytes(&png_bytes)
        .map_err(|err| io::Error::other(format!("Pix::from_bytes failed: {err}")))?;

    let extract_params = ExtractParams {
        pt_x: crop.pt_x,
        pt_y: crop.pt_y,
        lookahead: profile.lookahead,
        lookbehind: profile.lookbehind,
        search_radius: profile.search_radius,
        vertical: false,
        scale_factor: OCR_SCALE_FACTOR_DEFAULT,
    };

    let extract = extract_text_block(&pix_crop, extract_params)
        .map_err(|err| io::Error::other(format!("extract_text_block failed: {err}")))?;

    let Some(result) = extract else {
        return Ok(None);
    };

    Ok(Some(BoundingBoxScreen {
        x: capture.monitor_x + crop.x + result.bbox_unscaled.x,
        y: capture.monitor_y + crop.y + result.bbox_unscaled.y,
        w: result.bbox_unscaled.w,
        h: result.bbox_unscaled.h,
    }))
}

pub fn mode_label(kind: HotkeyKind) -> &'static str {
    match kind {
        HotkeyKind::Q => "Q",
        HotkeyKind::W => "W",
        HotkeyKind::E => "E",
    }
}

#[derive(Debug)]
struct PrimaryCapture {
    image: RgbaImage,
    monitor_x: i32,
    monitor_y: i32,
    monitor_w: i32,
    monitor_h: i32,
}

#[derive(Debug, Clone, Copy)]
struct CropRegion {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    pt_x: i32,
    pt_y: i32,
}

fn capture_primary_monitor() -> io::Result<PrimaryCapture> {
    let mut monitors = Monitor::all().map_err(|err| io::Error::other(format!("list monitors failed: {err}")))?;
    if monitors.is_empty() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "no monitors available"));
    }

    let primary_index = monitors
        .iter()
        .position(|monitor| monitor.is_primary().unwrap_or(false))
        .unwrap_or(0);
    let monitor = monitors.swap_remove(primary_index);

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
    let image = monitor
        .capture_image()
        .map_err(|err| io::Error::other(format!("capture monitor failed: {err}")))?;

    Ok(PrimaryCapture {
        image,
        monitor_x,
        monitor_y,
        monitor_w,
        monitor_h,
    })
}

fn build_clamped_crop(
    local_x: i32,
    local_y: i32,
    profile: ModeProfile,
    monitor_w: i32,
    monitor_h: i32,
) -> Option<CropRegion> {
    let mut crop_x = local_x - profile.crop_left_offset;
    let mut crop_y = local_y - profile.crop_top_offset;
    let mut crop_w = profile.crop_w;
    let mut crop_h = profile.crop_h;
    let mut pt_x = profile.pt_in_crop_x;
    let mut pt_y = profile.pt_in_crop_y;

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

    Some(CropRegion {
        x: crop_x,
        y: crop_y,
        w: crop_w,
        h: crop_h,
        pt_x,
        pt_y,
    })
}

fn encode_png(image: &RgbaImage) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let encoder = PngEncoder::new(&mut bytes);
    encoder
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            ColorType::Rgba8.into(),
        )
        .map_err(|err| io::Error::other(format!("png encode failed: {err}")))?;
    Ok(bytes)
}
