use std::io;
use std::path::{Path, PathBuf};
use std::{env, fs};

use chrono::Local;
use image::RgbaImage;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ, SRCCOPY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};
use xcap::Monitor;

use crate::capture::{CursorPoint, HotkeyKind, ScreenRect};

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

pub fn capture_screen_rect(rect: ScreenRect) -> io::Result<Option<RgbaImage>> {
    let Some(clipped) = clip_screen_rect_to_virtual_desktop(rect)? else {
        return Ok(None);
    };

    capture_rect_via_gdi(clipped).map(Some)
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
    Ok(PathBuf::from(r"D:\Users\Home\Desktop\Capture2Text_Test"))
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

pub fn clip_screen_rect_to_virtual_desktop(rect: ScreenRect) -> io::Result<Option<ScreenRect>> {
    if rect.w <= 0 || rect.h <= 0 {
        return Ok(None);
    }

    let desktop = virtual_desktop_rect()?;
    let left = rect.x.max(desktop.x);
    let top = rect.y.max(desktop.y);
    let right = (rect.x + rect.w).min(desktop.x + desktop.w);
    let bottom = (rect.y + rect.h).min(desktop.y + desktop.h);

    if right <= left || bottom <= top {
        return Ok(None);
    }

    Ok(Some(ScreenRect {
        x: left,
        y: top,
        w: right - left,
        h: bottom - top,
    }))
}

fn virtual_desktop_rect() -> io::Result<ScreenRect> {
    let x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
    let y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
    let w = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
    let h = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };

    if w <= 0 || h <= 0 {
        return Err(io::Error::other("virtual desktop size is invalid"));
    }

    Ok(ScreenRect { x, y, w, h })
}

fn capture_rect_via_gdi(rect: ScreenRect) -> io::Result<RgbaImage> {
    let screen_dc = unsafe { GetDC(HWND::default()) };
    if screen_dc.0.is_null() {
        return Err(io::Error::other("GetDC failed"));
    }

    let mem_dc = unsafe { CreateCompatibleDC(screen_dc) };
    if mem_dc.0.is_null() {
        unsafe {
            let _ = ReleaseDC(HWND::default(), screen_dc);
        }
        return Err(io::Error::other("CreateCompatibleDC failed"));
    }

    let bitmap = unsafe { CreateCompatibleBitmap(screen_dc, rect.w, rect.h) };
    if bitmap.0.is_null() {
        unsafe {
            let _ = DeleteDC(mem_dc);
            let _ = ReleaseDC(HWND::default(), screen_dc);
        }
        return Err(io::Error::other("CreateCompatibleBitmap failed"));
    }

    let old_obj = unsafe { SelectObject(mem_dc, HGDIOBJ(bitmap.0)) };
    if old_obj.0.is_null() {
        unsafe {
            let _ = DeleteObject(bitmap);
            let _ = DeleteDC(mem_dc);
            let _ = ReleaseDC(HWND::default(), screen_dc);
        }
        return Err(io::Error::other("SelectObject failed"));
    }

    let result = (|| -> io::Result<RgbaImage> {
        if let Err(err) = unsafe { BitBlt(mem_dc, 0, 0, rect.w, rect.h, screen_dc, rect.x, rect.y, SRCCOPY) } {
            return Err(io::Error::other(format!("BitBlt failed: {err}")));
        }

        let mut bmi = BITMAPINFO::default();
        bmi.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: rect.w,
            biHeight: -rect.h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        };

        let mut buffer = vec![0_u8; (rect.w as usize) * (rect.h as usize) * 4];
        let lines = unsafe {
            GetDIBits(
                mem_dc,
                bitmap,
                0,
                rect.h as u32,
                Some(buffer.as_mut_ptr().cast()),
                &mut bmi,
                DIB_RGB_COLORS,
            )
        };

        if lines == 0 {
            return Err(io::Error::other("GetDIBits failed"));
        }

        for px in buffer.chunks_exact_mut(4) {
            px.swap(0, 2);
            px[3] = 255;
        }

        RgbaImage::from_raw(rect.w as u32, rect.h as u32, buffer)
            .ok_or_else(|| io::Error::other("RgbaImage::from_raw failed"))
    })();

    unsafe {
        let _ = SelectObject(mem_dc, old_obj);
        let _ = DeleteObject(bitmap);
        let _ = DeleteDC(mem_dc);
        let _ = ReleaseDC(HWND::default(), screen_dc);
    }

    result
}
