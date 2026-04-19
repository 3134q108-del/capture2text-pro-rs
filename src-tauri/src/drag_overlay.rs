use std::ffi::c_void;
use std::io;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use windows::core::w;
use windows::Win32::Foundation::{
    COLORREF, GetLastError, ERROR_CLASS_ALREADY_EXISTS, HINSTANCE, HWND, LPARAM, LRESULT, POINT,
    SIZE, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, HBITMAP, HDC,
    HGDIOBJ, SelectObject,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, CS_HREDRAW, CS_VREDRAW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    GetCursorPos, GetMessageW, GetPhysicalCursorPos, GetWindowLongPtrW, HMENU, HWND_TOPMOST,
    KillTimer, MSG, PostMessageW, PostQuitMessage, RegisterClassW,
    SET_WINDOW_POS_FLAGS, SetTimer, SetWindowLongPtrW, SetWindowPos, ShowWindow, TranslateMessage,
    ULW_ALPHA, UPDATE_LAYERED_WINDOW_FLAGS, UpdateLayeredWindow, WINDOW_EX_STYLE, WINDOW_STYLE,
    WM_APP, WM_DESTROY, WM_DISPLAYCHANGE, WM_TIMER, WNDCLASSW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP, GWLP_USERDATA,
    SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_SHOWWINDOW,
};

use crate::capture::pipeline::{MIN_OCR_HEIGHT, MIN_OCR_WIDTH};
use crate::capture::ScreenRect as CaptureScreenRect;
use crate::mouse_hook;

const DRAG_OVERLAY_CHANNEL_CAPACITY: usize = 8;
const DRAG_OVERLAY_INIT_TIMEOUT: Duration = Duration::from_secs(3);
const DRAG_OVERLAY_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
const DRAG_OVERLAY_FINALIZE_TIMEOUT: Duration = Duration::from_millis(200);

const WM_APP_CMD: u32 = WM_APP + 101;
const TIMER_ID: usize = 1;
const TIMER_INTERVAL_MS: u32 = 10;

const FILL_BGRA_PREMULT: u32 = premult_bgra(255, 0, 0, 64);
const BORDER_BGRA_PREMULT: u32 = premult_bgra(255, 0, 0, 255);

static DRAG_OVERLAY_RUNTIME: OnceLock<DragOverlayRuntime> = OnceLock::new();
static DRAG_OVERLAY_HWND_RAW: AtomicIsize = AtomicIsize::new(0);
static WARNED_NOT_INITIALIZED: AtomicBool = AtomicBool::new(false);
static DRAG_OVERLAY_ACTIVE: AtomicBool = AtomicBool::new(false);

struct DragOverlayRuntime {
    tx: SyncSender<DragOverlayCommand>,
    join: Mutex<Option<JoinHandle<()>>>,
}

enum DragOverlayCommand {
    BeginDrag,
    ShowRect(CaptureScreenRect),
    FinalizeAndGetRect(SyncSender<Option<CaptureScreenRect>>),
    Cancel,
    Exit,
}

enum DragMode {
    Idle,
    Growing { pivot: (i32, i32), current: (i32, i32) },
    Fixed,
}

#[derive(Clone, Copy)]
struct DragRect {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

struct DragOverlayContext {
    hwnd: HWND,
    hdc_mem: HDC,
    hbm: HBITMAP,
    old_bitmap: HGDIOBJ,
    bits_ptr: *mut u8,
    dib_w: i32,
    dib_h: i32,
    window_x: i32,
    window_y: i32,
    mode: DragMode,
    move_box_size: Option<(i32, i32)>,
    right_held_prev: bool,
}

pub fn init() -> io::Result<()> {
    if DRAG_OVERLAY_RUNTIME.get().is_some() {
        return Ok(());
    }

    let (cmd_tx, cmd_rx) = sync_channel(DRAG_OVERLAY_CHANNEL_CAPACITY);
    let (ready_tx, ready_rx) = sync_channel(0);

    let join = thread::Builder::new()
        .name("drag-overlay-thread".to_string())
        .spawn(move || drag_overlay_thread_main(cmd_rx, ready_tx))
        .map_err(|err| io::Error::other(format!("spawn drag overlay thread failed: {err}")))?;

    let ready_result = ready_rx
        .recv_timeout(DRAG_OVERLAY_INIT_TIMEOUT)
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "drag overlay init handshake timed out"))?;

    if let Err(err) = ready_result {
        let _ = join.join();
        return Err(err);
    }

    DRAG_OVERLAY_RUNTIME
        .set(DragOverlayRuntime {
            tx: cmd_tx,
            join: Mutex::new(Some(join)),
        })
        .map_err(|_| io::Error::new(io::ErrorKind::AlreadyExists, "drag overlay runtime already initialized"))?;

    Ok(())
}

pub fn begin_drag() {
    send_command(DragOverlayCommand::BeginDrag);
}

#[allow(dead_code)]
pub fn show(rect: CaptureScreenRect) {
    send_command(DragOverlayCommand::ShowRect(rect));
}

pub fn cancel() {
    send_command(DragOverlayCommand::Cancel);
}

pub fn finalize_and_get_rect() -> Option<CaptureScreenRect> {
    let runtime = DRAG_OVERLAY_RUNTIME.get()?;
    let (tx, rx) = sync_channel(1);

    runtime
        .tx
        .try_send(DragOverlayCommand::FinalizeAndGetRect(tx))
        .ok()?;
    wake_drag_overlay_thread();

    rx.recv_timeout(DRAG_OVERLAY_FINALIZE_TIMEOUT).ok().flatten()
}

pub fn is_active() -> bool {
    DRAG_OVERLAY_ACTIVE.load(Ordering::Relaxed)
}

pub fn shutdown() {
    let Some(runtime) = DRAG_OVERLAY_RUNTIME.get() else {
        return;
    };

    wake_drag_overlay_thread();
    let _ = runtime.tx.send(DragOverlayCommand::Exit);
    wake_drag_overlay_thread();

    let join = runtime.join.lock().ok().and_then(|mut guard| guard.take());
    if let Some(join) = join {
        let (done_tx, done_rx) = sync_channel(1);
        let _ = thread::Builder::new()
            .name("drag-overlay-join-wait".to_string())
            .spawn(move || {
                let _ = join.join();
                let _ = done_tx.send(());
            });
        let _ = done_rx.recv_timeout(DRAG_OVERLAY_SHUTDOWN_TIMEOUT);
    }
}

fn send_command(command: DragOverlayCommand) {
    let Some(runtime) = DRAG_OVERLAY_RUNTIME.get() else {
        if !WARNED_NOT_INITIALIZED.swap(true, Ordering::Relaxed) {
            eprintln!("[drag_overlay] drag overlay not initialized");
        }
        return;
    };

    let _ = runtime.tx.try_send(command);
    wake_drag_overlay_thread();
}

fn drag_overlay_thread_main(rx: Receiver<DragOverlayCommand>, ready_tx: SyncSender<io::Result<()>>) {
    if let Err(err) = run_drag_overlay_thread(rx, &ready_tx) {
        let _ = ready_tx.send(Err(err));
    }
}

fn run_drag_overlay_thread(
    rx: Receiver<DragOverlayCommand>,
    ready_tx: &SyncSender<io::Result<()>>,
) -> io::Result<()> {
    let hinstance = unsafe { GetModuleHandleW(None) }
        .map(|hmodule| HINSTANCE(hmodule.0))
        .map_err(|err| io::Error::other(format!("GetModuleHandleW failed: {err}")))?;

    register_drag_overlay_class(hinstance)?;
    let hwnd = create_drag_overlay_window(hinstance)?;

    let hdc_mem = unsafe { CreateCompatibleDC(None) };
    if hdc_mem.0.is_null() {
        return Err(io::Error::other("CreateCompatibleDC failed"));
    }

    let mut context = Box::new(DragOverlayContext {
        hwnd,
        hdc_mem,
        hbm: HBITMAP::default(),
        old_bitmap: HGDIOBJ::default(),
        bits_ptr: ptr::null_mut(),
        dib_w: 0,
        dib_h: 0,
        window_x: 0,
        window_y: 0,
        mode: DragMode::Idle,
        move_box_size: None,
        right_held_prev: false,
    });

    ensure_dib_size(&mut context, 1, 1)?;
    draw_box(&context, 1, 1)?;
    update_layered(&context)?;

    unsafe {
        let _ = SetWindowLongPtrW(
            context.hwnd,
            GWLP_USERDATA,
            (&mut *context as *mut DragOverlayContext) as isize,
        );
    }

    DRAG_OVERLAY_HWND_RAW.store(hwnd.0 as isize, Ordering::Relaxed);
    let _ = ready_tx.send(Ok(()));

    let mut msg = MSG::default();
    loop {
        let status = unsafe { GetMessageW(&mut msg, None, 0, 0) }.0;
        if status == -1 || status == 0 {
            break;
        }

        if msg.message == WM_APP_CMD {
            if !drain_commands(&rx, &mut context) {
                break;
            }
            continue;
        }

        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    unsafe {
        let _ = SetWindowLongPtrW(context.hwnd, GWLP_USERDATA, 0);
    }
    cleanup_context(&mut context);
    DRAG_OVERLAY_ACTIVE.store(false, Ordering::Relaxed);
    DRAG_OVERLAY_HWND_RAW.store(0, Ordering::Relaxed);
    Ok(())
}

fn drain_commands(rx: &Receiver<DragOverlayCommand>, context: &mut DragOverlayContext) -> bool {
    while let Ok(command) = rx.try_recv() {
        let result = match command {
            DragOverlayCommand::BeginDrag => begin_grow_mode(context),
            DragOverlayCommand::ShowRect(rect) => begin_fixed_mode(context, rect),
            DragOverlayCommand::FinalizeAndGetRect(reply_tx) => {
                let rect = finalize_rect(context);
                let _ = reply_tx.send(rect);
                Ok(())
            }
            DragOverlayCommand::Cancel => hide_overlay(context),
            DragOverlayCommand::Exit => {
                let _ = hide_overlay(context);
                unsafe {
                    let _ = DestroyWindow(context.hwnd);
                }
                return false;
            }
        };

        if result.is_err() {
            let _ = hide_overlay(context);
        }
    }

    true
}

fn begin_grow_mode(context: &mut DragOverlayContext) -> io::Result<()> {
    let cursor = cursor_pos().ok_or_else(|| io::Error::other("failed to read cursor position"))?;
    context.mode = DragMode::Growing {
        pivot: (cursor.x, cursor.y),
        current: (cursor.x, cursor.y),
    };
    context.move_box_size = None;
    context.right_held_prev = false;
    DRAG_OVERLAY_ACTIVE.store(true, Ordering::Relaxed);

    unsafe {
        let _ = SetTimer(context.hwnd, TIMER_ID, TIMER_INTERVAL_MS, None);
    }

    apply_rect(context, DragRect {
        x: cursor.x,
        y: cursor.y,
        w: 1,
        h: 1,
    })
}

fn begin_fixed_mode(context: &mut DragOverlayContext, rect: CaptureScreenRect) -> io::Result<()> {
    let normalized = normalize_rect(DragRect {
        x: rect.x,
        y: rect.y,
        w: rect.w,
        h: rect.h,
    });

    context.mode = DragMode::Fixed;
    context.move_box_size = None;
    context.right_held_prev = false;
    DRAG_OVERLAY_ACTIVE.store(true, Ordering::Relaxed);

    unsafe {
        let _ = KillTimer(context.hwnd, TIMER_ID);
    }

    apply_rect(context, normalized)
}

fn hide_overlay(context: &mut DragOverlayContext) -> io::Result<()> {
    context.mode = DragMode::Idle;
    context.move_box_size = None;
    context.right_held_prev = false;
    DRAG_OVERLAY_ACTIVE.store(false, Ordering::Relaxed);

    unsafe {
        let _ = KillTimer(context.hwnd, TIMER_ID);
        let _ = ShowWindow(context.hwnd, SW_HIDE);
    }

    Ok(())
}

fn finalize_rect(context: &mut DragOverlayContext) -> Option<CaptureScreenRect> {
    let rect = match context.mode {
        DragMode::Growing { pivot, current } => rect_from_points(pivot, current),
        DragMode::Fixed => DragRect {
            x: context.window_x,
            y: context.window_y,
            w: context.dib_w,
            h: context.dib_h,
        },
        DragMode::Idle => {
            let _ = hide_overlay(context);
            return None;
        }
    };

    let _ = hide_overlay(context);
    let rect = normalize_rect(rect);
    if rect.w < MIN_OCR_WIDTH || rect.h < MIN_OCR_HEIGHT {
        return None;
    }

    Some(CaptureScreenRect {
        x: rect.x,
        y: rect.y,
        w: rect.w,
        h: rect.h,
    })
}

fn tick_grow_mode(context: &mut DragOverlayContext) -> io::Result<()> {
    let DragMode::Growing { pivot, current } = context.mode else {
        return Ok(());
    };

    let right_held = mouse_hook::right_button_held();
    let current_rect = normalize_rect(rect_from_points(pivot, current));
    if right_held && !context.right_held_prev {
        context.move_box_size = Some((current_rect.w, current_rect.h));
    } else if !right_held && context.right_held_prev {
        context.move_box_size = None;
    }
    context.right_held_prev = right_held;

    let Some(cursor) = cursor_pos() else {
        return Ok(());
    };

    let current_xy = (cursor.x, cursor.y);
    if current_xy == current && !right_held {
        return Ok(());
    }

    let mut next_pivot = pivot;
    let next_current = current_xy;

    if right_held {
        let (box_w, box_h) = context
            .move_box_size
            .unwrap_or((current_rect.w.max(1), current_rect.h.max(1)));

        if current_xy.0 < next_pivot.0 {
            let x1 = current_xy.0;
            next_pivot.0 = x1 + box_w;
        } else {
            let x2 = current_xy.0;
            next_pivot.0 = x2 - box_w;
        }

        if current_xy.1 < next_pivot.1 {
            let y1 = current_xy.1;
            next_pivot.1 = y1 + box_h;
        } else {
            let y2 = current_xy.1;
            next_pivot.1 = y2 - box_h;
        }
    }

    context.mode = DragMode::Growing {
        pivot: next_pivot,
        current: next_current,
    };

    apply_rect(context, rect_from_points(next_pivot, next_current))
}

fn rect_from_points(a: (i32, i32), b: (i32, i32)) -> DragRect {
    let x1 = a.0.min(b.0);
    let y1 = a.1.min(b.1);
    let x2 = a.0.max(b.0);
    let y2 = a.1.max(b.1);

    DragRect {
        x: x1,
        y: y1,
        w: (x2 - x1).max(0) + 1,
        h: (y2 - y1).max(0) + 1,
    }
}

fn normalize_rect(rect: DragRect) -> DragRect {
    let x2 = rect.x + rect.w;
    let y2 = rect.y + rect.h;
    let x1 = rect.x.min(x2);
    let y1 = rect.y.min(y2);
    let x2 = rect.x.max(x2);
    let y2 = rect.y.max(y2);

    DragRect {
        x: x1,
        y: y1,
        w: (x2 - x1).max(1),
        h: (y2 - y1).max(1),
    }
}

fn apply_rect(context: &mut DragOverlayContext, rect: DragRect) -> io::Result<()> {
    let rect = normalize_rect(rect);
    context.window_x = rect.x;
    context.window_y = rect.y;

    let flags = SET_WINDOW_POS_FLAGS(SWP_NOACTIVATE.0 | SWP_SHOWWINDOW.0);
    unsafe {
        SetWindowPos(
            context.hwnd,
            HWND_TOPMOST,
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            flags,
        )
    }
    .map_err(|err| io::Error::other(format!("SetWindowPos failed: {err}")))?;

    ensure_dib_size(context, rect.w, rect.h)?;
    draw_box(context, rect.w, rect.h)?;
    update_layered(context)?;

    unsafe {
        let _ = ShowWindow(context.hwnd, SW_SHOWNOACTIVATE);
    }

    Ok(())
}

fn cursor_pos() -> Option<POINT> {
    let mut point = POINT::default();
    if unsafe { GetPhysicalCursorPos(&mut point) }.is_ok() {
        return Some(point);
    }
    if unsafe { GetCursorPos(&mut point) }.is_ok() {
        return Some(point);
    }
    None
}

fn register_drag_overlay_class(hinstance: HINSTANCE) -> io::Result<()> {
    let wnd_class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(drag_overlay_wnd_proc),
        hInstance: hinstance,
        lpszClassName: w!("Capture2TextDragOverlayWindow"),
        ..Default::default()
    };

    let atom = unsafe { RegisterClassW(&wnd_class) };
    if atom == 0 {
        let err = unsafe { GetLastError() };
        if err != ERROR_CLASS_ALREADY_EXISTS {
            return Err(io::Error::other(format!("RegisterClassW failed: {err:?}")));
        }
    }

    Ok(())
}

fn create_drag_overlay_window(hinstance: HINSTANCE) -> io::Result<HWND> {
    let ex_style = WINDOW_EX_STYLE(
        WS_EX_LAYERED.0
            | WS_EX_TOPMOST.0
            | WS_EX_TOOLWINDOW.0
            | WS_EX_NOACTIVATE.0
            | WS_EX_TRANSPARENT.0,
    );
    let style = WINDOW_STYLE(WS_POPUP.0);

    unsafe {
        CreateWindowExW(
            ex_style,
            w!("Capture2TextDragOverlayWindow"),
            w!("Capture2TextDragOverlayWindow"),
            style,
            0,
            0,
            1,
            1,
            HWND::default(),
            HMENU::default(),
            hinstance,
            None,
        )
    }
    .map_err(|err| io::Error::other(format!("CreateWindowExW failed: {err}")))
}

fn ensure_dib_size(context: &mut DragOverlayContext, w: i32, h: i32) -> io::Result<()> {
    if context.dib_w == w && context.dib_h == h && !context.hbm.0.is_null() {
        return Ok(());
    }

    destroy_dib(context);

    let mut bmi = BITMAPINFO::default();
    bmi.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: w,
        biHeight: -h,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };

    let mut bits: *mut c_void = ptr::null_mut();
    let hbm = unsafe { CreateDIBSection(context.hdc_mem, &bmi, DIB_RGB_COLORS, &mut bits, None, 0) }
        .map_err(|err| io::Error::other(format!("CreateDIBSection failed: {err}")))?;

    let previous = unsafe { SelectObject(context.hdc_mem, HGDIOBJ(hbm.0)) };
    if previous.0.is_null() {
        unsafe {
            let _ = DeleteObject(hbm);
        }
        return Err(io::Error::other("SelectObject failed"));
    }

    if context.old_bitmap.0.is_null() {
        context.old_bitmap = previous;
    }

    context.hbm = hbm;
    context.bits_ptr = bits as *mut u8;
    context.dib_w = w;
    context.dib_h = h;

    Ok(())
}

fn destroy_dib(context: &mut DragOverlayContext) {
    if context.hbm.0.is_null() {
        return;
    }

    unsafe {
        if !context.old_bitmap.0.is_null() {
            let _ = SelectObject(context.hdc_mem, context.old_bitmap);
        }
        let _ = DeleteObject(context.hbm);
    }

    context.hbm = HBITMAP::default();
    context.bits_ptr = ptr::null_mut();
    context.dib_w = 0;
    context.dib_h = 0;
}

fn pixels_mut(context: &DragOverlayContext) -> io::Result<&mut [u32]> {
    if context.bits_ptr.is_null() || context.dib_w <= 0 || context.dib_h <= 0 {
        return Err(io::Error::other("drag overlay dib is not initialized"));
    }

    let len = (context.dib_w as usize) * (context.dib_h as usize);
    Ok(unsafe { std::slice::from_raw_parts_mut(context.bits_ptr as *mut u32, len) })
}

fn draw_box(context: &DragOverlayContext, w: i32, h: i32) -> io::Result<()> {
    let pixels = pixels_mut(context)?;
    pixels.fill(FILL_BGRA_PREMULT);

    if w <= 1 || h <= 1 {
        pixels.fill(BORDER_BGRA_PREMULT);
        return Ok(());
    }

    let pitch = w as usize;
    let top = 0usize;
    let bottom = (h - 1) as usize;
    let left = 0usize;
    let right = (w - 1) as usize;

    let top_start = top * pitch;
    let top_end = top_start + w as usize;
    pixels[top_start..top_end].fill(BORDER_BGRA_PREMULT);

    let bottom_start = bottom * pitch;
    let bottom_end = bottom_start + w as usize;
    pixels[bottom_start..bottom_end].fill(BORDER_BGRA_PREMULT);

    for y in 1..(h - 1) {
        let row_start = (y as usize) * pitch;
        pixels[row_start + left] = BORDER_BGRA_PREMULT;
        pixels[row_start + right] = BORDER_BGRA_PREMULT;
    }

    Ok(())
}

const fn premult_channel(channel: u8, alpha: u8) -> u8 {
    ((channel as u16 * alpha as u16 + 127) / 255) as u8
}

const fn premult_bgra(r: u8, g: u8, b: u8, a: u8) -> u32 {
    u32::from_le_bytes([
        premult_channel(b, a),
        premult_channel(g, a),
        premult_channel(r, a),
        a,
    ])
}

fn update_layered(context: &DragOverlayContext) -> io::Result<()> {
    let dst = POINT {
        x: context.window_x,
        y: context.window_y,
    };
    let size = SIZE {
        cx: context.dib_w,
        cy: context.dib_h,
    };
    let src = POINT { x: 0, y: 0 };
    let blend = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: AC_SRC_ALPHA as u8,
    };

    unsafe {
        UpdateLayeredWindow(
            context.hwnd,
            None,
            Some(&dst),
            Some(&size),
            context.hdc_mem,
            Some(&src),
            COLORREF(0),
            Some(&blend),
            UPDATE_LAYERED_WINDOW_FLAGS(ULW_ALPHA.0),
        )
    }
    .map_err(|err| io::Error::other(format!("UpdateLayeredWindow failed: {err}")))
}

fn cleanup_context(context: &mut DragOverlayContext) {
    let _ = hide_overlay(context);
    destroy_dib(context);

    if !context.hdc_mem.0.is_null() {
        unsafe {
            let _ = DeleteDC(context.hdc_mem);
        }
        context.hdc_mem = HDC::default();
    }

    context.old_bitmap = HGDIOBJ::default();
}

fn wake_drag_overlay_thread() {
    let hwnd_raw = DRAG_OVERLAY_HWND_RAW.load(Ordering::Relaxed);
    if hwnd_raw == 0 {
        return;
    }

    let hwnd = HWND(hwnd_raw as *mut c_void);
    let _ = unsafe { PostMessageW(hwnd, WM_APP_CMD, WPARAM(0), LPARAM(0)) };
}

extern "system" fn drag_overlay_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TIMER => {
            if wparam.0 == TIMER_ID {
                let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut DragOverlayContext;
                if !ptr.is_null() {
                    let context = unsafe { &mut *ptr };
                    let _ = tick_grow_mode(context);
                }
            }
            LRESULT(0)
        }
        WM_DISPLAYCHANGE => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut DragOverlayContext;
            if !ptr.is_null() {
                let context = unsafe { &mut *ptr };
                let _ = ensure_dib_size(context, context.dib_w.max(1), context.dib_h.max(1));
                let _ = update_layered(context);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe {
                PostQuitMessage(0);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
