use std::io;
use std::time::Instant;
use std::env;

use image::codecs::png::PngEncoder;
use image::{ColorType, ImageEncoder, RgbaImage};

use crate::capture::preprocess::{extract_text_block, ExtractParams, OCR_SCALE_FACTOR_DEFAULT};
use crate::capture::screen_capture::{
    capture_at_cursor, capture_screen_rect, clip_screen_rect_to_virtual_desktop, maybe_debug_save_capture,
    CropRequest,
};
use crate::capture::params::profile_for;
use crate::capture::{CaptureRequest, CursorPoint, HotkeyKind, ScreenRect};
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

pub fn run_for_request(request: CaptureRequest) -> io::Result<Option<BoundingBoxScreen>> {
    match request {
        CaptureRequest::Hotkey {
            kind,
            cursor,
            queued_at,
        } => run_for_hotkey_event(kind, cursor, queued_at),
        CaptureRequest::SelectedRect { rect, queued_at } => run_for_selected_rect(rect, queued_at),
    }
}

fn run_for_hotkey_event(
    kind: HotkeyKind,
    cursor: CursorPoint,
    queued_at: Instant,
) -> io::Result<Option<BoundingBoxScreen>> {
    let t0 = Instant::now();
    let perf = perf_enabled();

    let Some(profile) = profile_for(kind) else {
        return Ok(None);
    };

    let t_cap_start = Instant::now();
    let Some(capture) = capture_at_cursor(
        cursor,
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
    let t_cap = t_cap_start.elapsed();

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

    maybe_debug_save_capture(kind, &capture.image);

    let t_enc_start = Instant::now();
    let png_bytes = encode_png(&capture.image)?;
    let t_enc = t_enc_start.elapsed();

    let t_pix_start = Instant::now();
    let pix_crop = Pix::from_bytes(&png_bytes)
        .map_err(|err| io::Error::other(format!("Pix::from_bytes failed: {err}")))?;
    let t_pix = t_pix_start.elapsed();

    let extract_params = ExtractParams {
        pt_x: capture.pt_x,
        pt_y: capture.pt_y,
        lookahead: profile.lookahead,
        lookbehind: profile.lookbehind,
        search_radius: profile.search_radius,
        vertical: false,
        scale_factor: OCR_SCALE_FACTOR_DEFAULT,
    };

    let t_ext_start = Instant::now();
    let extract = extract_text_block(&pix_crop, extract_params)
        .map_err(|err| io::Error::other(format!("extract_text_block failed: {err}")))?;
    let t_ext = t_ext_start.elapsed();
    let t_queue = queued_at.elapsed();
    let t_total = t0.elapsed();

    let Some(result) = extract else {
        if perf {
            print_perf(kind, t_queue, t_cap, t_enc, t_pix, t_ext, t_total);
        }
        return Ok(None);
    };

    // MainWindow::minOcrWidth/Height (=3) OR-check on unscaled bbox
    if result.bbox_unscaled.w < MIN_OCR_WIDTH || result.bbox_unscaled.h < MIN_OCR_HEIGHT {
        if perf {
            print_perf(kind, t_queue, t_cap, t_enc, t_pix, t_ext, t_total);
        }
        return Ok(None);
    }

    if perf {
        print_perf(kind, t_queue, t_cap, t_enc, t_pix, t_ext, t_total);
    }

    Ok(Some(BoundingBoxScreen {
        x: capture.monitor_x + capture.crop_x + result.bbox_unscaled.x + 1,
        y: capture.monitor_y + capture.crop_y + result.bbox_unscaled.y + 1,
        w: result.bbox_unscaled.w,
        h: result.bbox_unscaled.h,
    }))
}

fn run_for_selected_rect(rect: ScreenRect, queued_at: Instant) -> io::Result<Option<BoundingBoxScreen>> {
    let t0 = Instant::now();
    let perf = perf_enabled();

    let Some(clipped_rect) = clip_screen_rect_to_virtual_desktop(rect)? else {
        println!("[pipeline] selected rect outside virtual desktop, skip");
        return Ok(None);
    };

    let Some(image) = capture_screen_rect(clipped_rect)? else {
        println!("[pipeline] selected rect capture returned empty, skip");
        return Ok(None);
    };

    if clipped_rect.w < MIN_OCR_WIDTH || clipped_rect.h < MIN_OCR_HEIGHT {
        return Ok(None);
    }

    maybe_debug_save_capture(HotkeyKind::Q, &image);

    if perf {
        println!(
            "[perf] mode=Q q={}ms cap={}ms total={}ms",
            queued_at.elapsed().as_millis(),
            t0.elapsed().as_millis(),
            t0.elapsed().as_millis()
        );
    }

    Ok(Some(BoundingBoxScreen {
        x: clipped_rect.x,
        y: clipped_rect.y,
        w: clipped_rect.w,
        h: clipped_rect.h,
    }))
}

pub fn mode_label(kind: HotkeyKind) -> &'static str {
    match kind {
        HotkeyKind::Q => "Q",
        HotkeyKind::W => "W",
        HotkeyKind::E => "E",
    }
}

pub fn request_label(request: CaptureRequest) -> &'static str {
    match request {
        CaptureRequest::Hotkey { kind, .. } => mode_label(kind),
        CaptureRequest::SelectedRect { .. } => "Q",
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

fn perf_enabled() -> bool {
    matches!(env::var("C2T_PERF").ok().as_deref(), Some("1"))
}

fn print_perf(
    kind: HotkeyKind,
    t_queue: std::time::Duration,
    t_cap: std::time::Duration,
    t_enc: std::time::Duration,
    t_pix: std::time::Duration,
    t_ext: std::time::Duration,
    t_total: std::time::Duration,
) {
    println!(
        "[perf] mode={} q={}ms cap={}ms enc={}ms pix={}ms ext={}ms total={}ms",
        mode_label(kind),
        t_queue.as_millis(),
        t_cap.as_millis(),
        t_enc.as_millis(),
        t_pix.as_millis(),
        t_ext.as_millis(),
        t_total.as_millis()
    );
}
