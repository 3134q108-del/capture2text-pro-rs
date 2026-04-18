use std::io;

use image::codecs::png::PngEncoder;
use image::{ColorType, ImageEncoder, RgbaImage};

use crate::capture::preprocess::{extract_text_block, ExtractParams, OCR_SCALE_FACTOR_DEFAULT};
use crate::capture::screen_capture::{capture_at_cursor, maybe_debug_save_capture, CropRequest};
use crate::capture::{HotkeyEvent, HotkeyKind};
use crate::capture::params::profile_for;
use crate::leptonica::Pix;

pub const MIN_OCR_WIDTH: i32 = 3;
pub const MIN_OCR_HEIGHT: i32 = 3;

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

    let Some(capture) = capture_at_cursor(
        event.cursor,
        CropRequest {
            crop_left_offset: profile.crop_left_offset,
            crop_top_offset: profile.crop_top_offset,
            crop_w: profile.crop_w,
            crop_h: profile.crop_h,
            pt_in_crop_x: profile.pt_in_crop_x,
            pt_in_crop_y: profile.pt_in_crop_y,
        },
    )? else {
        return Ok(None);
    };

    let crop_w = capture.image.width() as i32;
    let crop_h = capture.image.height() as i32;
    if capture.crop_x < 0
        || capture.crop_y < 0
        || crop_w <= 0
        || crop_h <= 0
        || capture.crop_x + crop_w > capture.monitor_w
        || capture.crop_y + crop_h > capture.monitor_h
    {
        return Ok(None);
    }

    maybe_debug_save_capture(event.kind, &capture.image);

    let png_bytes = encode_png(&capture.image)?;
    let pix_crop = Pix::from_bytes(&png_bytes)
        .map_err(|err| io::Error::other(format!("Pix::from_bytes failed: {err}")))?;

    let extract_params = ExtractParams {
        pt_x: capture.pt_x,
        pt_y: capture.pt_y,
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

    // MainWindow::minOcrWidth/Height (=3) OR-check on unscaled bbox
    if result.bbox_unscaled.w < MIN_OCR_WIDTH || result.bbox_unscaled.h < MIN_OCR_HEIGHT {
        return Ok(None);
    }

    Ok(Some(BoundingBoxScreen {
        x: capture.monitor_x + capture.crop_x + result.bbox_unscaled.x + 1,
        y: capture.monitor_y + capture.crop_y + result.bbox_unscaled.y + 1,
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
