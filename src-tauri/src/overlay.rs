use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tauri::{
    AppHandle, Manager, PhysicalPosition, PhysicalSize, Position, Size, WebviewUrl,
    WebviewWindowBuilder,
};

use crate::capture::pipeline::BoundingBoxScreen;

static OVERLAY_GENERATION: AtomicU64 = AtomicU64::new(0);

pub fn init(app: &AppHandle) -> tauri::Result<()> {
    if app.get_webview_window("overlay").is_some() {
        return Ok(());
    }

    let window = WebviewWindowBuilder::new(app, "overlay", WebviewUrl::App("overlay.html".into()))
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .shadow(false)
        .visible(false)
        .maximizable(false)
        .minimizable(false)
        .closable(false)
        .build()?;

    window.set_ignore_cursor_events(true)?;
    Ok(())
}

pub fn show(app: &AppHandle, bbox: BoundingBoxScreen) {
    if bbox.w <= 0 || bbox.h <= 0 {
        return;
    }

    let Some(window) = app.get_webview_window("overlay") else {
        return;
    };

    let position = Position::Physical(PhysicalPosition::new(bbox.x - 1, bbox.y - 1));
    let size = Size::Physical(PhysicalSize::new((bbox.w + 2) as u32, (bbox.h + 2) as u32));

    if let Err(err) = window.set_position(position) {
        eprintln!("[overlay] set_position failed: {err}");
        return;
    }

    if let Err(err) = window.set_size(size) {
        eprintln!("[overlay] set_size failed: {err}");
        return;
    }

    let my_generation = OVERLAY_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;

    if let Err(err) = window.show() {
        eprintln!("[overlay] show failed: {err}");
        return;
    }

    tauri::async_runtime::spawn(async move {
        let _ = tauri::async_runtime::spawn_blocking(move || {
            std::thread::sleep(Duration::from_millis(500));
        })
        .await;

        if OVERLAY_GENERATION.load(Ordering::SeqCst) != my_generation {
            return;
        }

        if let Err(err) = window.hide() {
            eprintln!("[overlay] hide failed: {err}");
        }
    });
}
