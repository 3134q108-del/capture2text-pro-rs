use std::ffi::c_void;
use std::io;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

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
use windows::Win32::UI::HiDpi::{
    GetAwarenessFromDpiAwarenessContext, GetThreadDpiAwarenessContext, GetWindowDpiAwarenessContext,
    SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetCapture, ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, CS_HREDRAW, CS_VREDRAW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    GetMessageW, GetSystemMetrics, GetWindowLongPtrW, HMENU, HWND_TOPMOST, IDC_CROSS, LoadCursorW,
    MA_NOACTIVATE, MSG, PostMessageW, PostQuitMessage, RegisterClassW, SET_WINDOW_POS_FLAGS,
    SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SW_HIDE,
    SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetCursor, SetWindowLongPtrW, SetWindowPos,
    ShowWindow, TranslateMessage, ULW_ALPHA, UPDATE_LAYERED_WINDOW_FLAGS, UpdateLayeredWindow,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_DESTROY, WM_DISPLAYCHANGE, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_SETCURSOR, WNDCLASSW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, GWLP_USERDATA,
};

use crate::capture::{self, CaptureRequest, ScreenRect as CaptureScreenRect};

const DRAG_OVERLAY_CHANNEL_CAPACITY: usize = 8;
const DRAG_OVERLAY_INIT_TIMEOUT: Duration = Duration::from_secs(3);
const DRAG_OVERLAY_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

const WM_APP_CMD: u32 = WM_APP + 101;
const WM_APP_RENDER: u32 = WM_APP + 102;

const FILL_BGRA_PREMULT: u32 = u32::from_le_bytes([60, 30, 0, 60]);
const BORDER_BGRA_PREMULT: u32 = u32::from_le_bytes([255, 128, 0, 255]);

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
    Cancel,
    Exit,
}

enum DragState {
    Idle,
    Waiting,
    Dragging { start: (i32, i32), current: (i32, i32) },
}

#[derive(Clone, Copy)]
struct DragRect {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

#[derive(Clone, Copy)]
struct ClippedRect {
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
    desktop_x: i32,
    desktop_y: i32,
    desktop_w: i32,
    desktop_h: i32,
    state: DragState,
    pending_render: bool,
    pending_resize: bool,
    last_drawn_rect: Option<DragRect>,
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

pub fn cancel() {
    send_command(DragOverlayCommand::Cancel);
}

pub fn is_active() -> bool {
    DRAG_OVERLAY_ACTIVE.load(Ordering::Relaxed)
}

pub fn shutdown() {
    let Some(runtime) = DRAG_OVERLAY_RUNTIME.get() else {
        return;
    };

    let _ = runtime.tx.try_send(DragOverlayCommand::Exit);
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
    try_set_thread_dpi_awareness();

    let hinstance = unsafe { GetModuleHandleW(None) }
        .map(|hmodule| HINSTANCE(hmodule.0))
        .map_err(|err| io::Error::other(format!("GetModuleHandleW failed: {err}")))?;

    register_drag_overlay_class(hinstance)?;

    let (desktop_x, desktop_y, desktop_w, desktop_h) = query_virtual_desktop()?;
    let hwnd = create_drag_overlay_window(hinstance, desktop_x, desktop_y, desktop_w, desktop_h)?;

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
        desktop_x,
        desktop_y,
        desktop_w,
        desktop_h,
        state: DragState::Idle,
        pending_render: false,
        pending_resize: false,
        last_drawn_rect: None,
    });

    recreate_dib(&mut context)?;
    clear_dib(&context)?;
    update_layered(&context)?;
    log_window_dpi_awareness(hwnd);

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
    DRAG_OVERLAY_HWND_RAW.store(0, Ordering::Relaxed);
    Ok(())
}

fn drain_commands(rx: &Receiver<DragOverlayCommand>, context: &mut DragOverlayContext) -> bool {
    while let Ok(command) = rx.try_recv() {
        match command {
            DragOverlayCommand::BeginDrag => {
                if matches!(context.state, DragState::Idle) {
                    if let Err(err) = ensure_virtual_desktop_up_to_date(context) {
                        eprintln!("[drag_overlay] ensure desktop metrics failed: {err}");
                        continue;
                    }

                    if let Err(err) = clear_dib(context).and_then(|_| update_layered(context)) {
                        eprintln!("[drag_overlay] clear before begin failed: {err}");
                    }

                    context.state = DragState::Waiting;
                    context.pending_render = false;
                    context.last_drawn_rect = None;
                    DRAG_OVERLAY_ACTIVE.store(true, Ordering::Relaxed);

                    let flags = SET_WINDOW_POS_FLAGS(SWP_NOACTIVATE.0 | SWP_SHOWWINDOW.0);
                    if let Err(err) = unsafe {
                        SetWindowPos(
                            context.hwnd,
                            HWND_TOPMOST,
                            context.desktop_x,
                            context.desktop_y,
                            context.desktop_w,
                            context.desktop_h,
                            flags,
                        )
                    } {
                        eprintln!("[drag_overlay] SetWindowPos failed: {err}");
                    }

                    unsafe {
                        let _ = ShowWindow(context.hwnd, SW_SHOWNOACTIVATE);
                    }
                }
            }
            DragOverlayCommand::Cancel => {
                if !matches!(context.state, DragState::Idle) {
                    if let Err(err) = hide_and_reset_overlay(context) {
                        eprintln!("[drag_overlay] cancel failed: {err}");
                    }
                }
            }
            DragOverlayCommand::Exit => {
                if let Err(err) = hide_and_reset_overlay(context) {
                    eprintln!("[drag_overlay] exit cleanup failed: {err}");
                }
                unsafe {
                    let _ = DestroyWindow(context.hwnd);
                }
                return false;
            }
        }
    }
    true
}

fn try_set_thread_dpi_awareness() {
    let previous = unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    if previous.0.is_null() {
        eprintln!("[drag_overlay] SetThreadDpiAwarenessContext(PMv2) failed");
    } else {
        eprintln!(
            "[drag_overlay] thread DPI awareness now {}",
            awareness_name_from_context(unsafe { GetThreadDpiAwarenessContext() })
        );
    }
}

fn log_window_dpi_awareness(hwnd: HWND) {
    let awareness = unsafe { GetWindowDpiAwarenessContext(hwnd) };
    eprintln!(
        "[drag_overlay] window DPI awareness={}",
        awareness_name_from_context(awareness)
    );
}

fn awareness_name_from_context(context: windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT) -> &'static str {
    match unsafe { GetAwarenessFromDpiAwarenessContext(context) }.0 {
        -1 => "Invalid",
        0 => "Unaware",
        1 => "SystemAware",
        2 => "PerMonitorAware",
        _ => "Unknown",
    }
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

fn create_drag_overlay_window(
    hinstance: HINSTANCE,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> io::Result<HWND> {
    let ex_style =
        WINDOW_EX_STYLE(WS_EX_LAYERED.0 | WS_EX_TOPMOST.0 | WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0);
    let style = WINDOW_STYLE(WS_POPUP.0);

    unsafe {
        CreateWindowExW(
            ex_style,
            w!("Capture2TextDragOverlayWindow"),
            w!("Capture2TextDragOverlayWindow"),
            style,
            x,
            y,
            w,
            h,
            HWND::default(),
            HMENU::default(),
            hinstance,
            None,
        )
    }
    .map_err(|err| io::Error::other(format!("CreateWindowExW failed: {err}")))
}

fn query_virtual_desktop() -> io::Result<(i32, i32, i32, i32)> {
    let x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
    let y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
    let w = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
    let h = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };

    if w <= 0 || h <= 0 {
        return Err(io::Error::other("virtual desktop size is invalid"));
    }
    Ok((x, y, w, h))
}

fn ensure_virtual_desktop_up_to_date(context: &mut DragOverlayContext) -> io::Result<()> {
    if !context.pending_resize {
        return Ok(());
    }

    let (x, y, w, h) = query_virtual_desktop()?;
    context.desktop_x = x;
    context.desktop_y = y;
    context.desktop_w = w;
    context.desktop_h = h;
    context.pending_resize = false;
    context.last_drawn_rect = None;

    recreate_dib(context)?;
    clear_dib(context)?;
    update_layered(context)?;
    Ok(())
}

fn recreate_dib(context: &mut DragOverlayContext) -> io::Result<()> {
    destroy_dib(context);

    let mut bmi = BITMAPINFO::default();
    bmi.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: context.desktop_w,
        biHeight: -context.desktop_h,
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
}

fn clear_dib(context: &DragOverlayContext) -> io::Result<()> {
    let pixels = pixels_mut(context)?;
    pixels.fill(0);
    Ok(())
}

fn pixels_mut(context: &DragOverlayContext) -> io::Result<&mut [u32]> {
    if context.bits_ptr.is_null() || context.desktop_w <= 0 || context.desktop_h <= 0 {
        return Err(io::Error::other("drag overlay dib is not initialized"));
    }

    let len = (context.desktop_w as usize) * (context.desktop_h as usize);
    Ok(unsafe { std::slice::from_raw_parts_mut(context.bits_ptr as *mut u32, len) })
}

fn update_layered(context: &DragOverlayContext) -> io::Result<()> {
    let dst = POINT {
        x: context.desktop_x,
        y: context.desktop_y,
    };
    let size = SIZE {
        cx: context.desktop_w,
        cy: context.desktop_h,
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
    destroy_dib(context);
    if !context.hdc_mem.0.is_null() {
        unsafe {
            let _ = DeleteDC(context.hdc_mem);
        }
    }

    context.hdc_mem = HDC::default();
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

fn post_render_if_needed(context: &mut DragOverlayContext) {
    if context.pending_render {
        return;
    }
    context.pending_render = true;
    let _ = unsafe { PostMessageW(context.hwnd, WM_APP_RENDER, WPARAM(0), LPARAM(0)) };
}

fn hide_and_reset_overlay(context: &mut DragOverlayContext) -> io::Result<()> {
    context.state = DragState::Idle;
    context.pending_render = false;
    DRAG_OVERLAY_ACTIVE.store(false, Ordering::Relaxed);

    if unsafe { GetCapture() } == context.hwnd {
        unsafe {
            let _ = ReleaseCapture();
        }
    }

    if context.last_drawn_rect.is_some() {
        render_current_state(context)?;
    }

    unsafe {
        let _ = ShowWindow(context.hwnd, SW_HIDE);
    }
    Ok(())
}

fn render_current_state(context: &mut DragOverlayContext) -> io::Result<()> {
    context.pending_render = false;

    if let Some(previous) = context.last_drawn_rect.take() {
        clear_rect(context, previous)?;
    }

    if let DragState::Dragging { start, current } = context.state {
        let rect = normalize_rect(start, current);
        if rect.w > 0 && rect.h > 0 {
            draw_capture_rect(context, rect)?;
            context.last_drawn_rect = Some(rect);
        }
    }

    update_layered(context)
}

fn clear_rect(context: &DragOverlayContext, rect: DragRect) -> io::Result<()> {
    let Some(clipped) = clip_to_desktop(context, rect) else {
        return Ok(());
    };

    let pixels = pixels_mut(context)?;
    for row in 0..clipped.h {
        let y = clipped.y + row;
        let row_start = (y as usize) * (context.desktop_w as usize) + (clipped.x as usize);
        let row_end = row_start + clipped.w as usize;
        pixels[row_start..row_end].fill(0);
    }

    Ok(())
}

fn draw_capture_rect(context: &DragOverlayContext, rect: DragRect) -> io::Result<()> {
    let Some(clipped) = clip_to_desktop(context, rect) else {
        return Ok(());
    };

    let pixels = pixels_mut(context)?;
    let pitch = context.desktop_w as usize;

    for row in 0..clipped.h {
        let y = clipped.y + row;
        let row_start = (y as usize) * pitch + (clipped.x as usize);
        let row_end = row_start + clipped.w as usize;
        pixels[row_start..row_end].fill(FILL_BGRA_PREMULT);
    }

    if clipped.w < 2 || clipped.h < 2 {
        for row in 0..clipped.h {
            let y = clipped.y + row;
            let row_start = (y as usize) * pitch + (clipped.x as usize);
            let row_end = row_start + clipped.w as usize;
            pixels[row_start..row_end].fill(BORDER_BGRA_PREMULT);
        }
        return Ok(());
    }

    let top = clipped.y as usize;
    let bottom = (clipped.y + clipped.h - 1) as usize;
    let left = clipped.x as usize;
    let right = (clipped.x + clipped.w - 1) as usize;

    let top_start = top * pitch + left;
    let top_end = top_start + clipped.w as usize;
    pixels[top_start..top_end].fill(BORDER_BGRA_PREMULT);

    let bottom_start = bottom * pitch + left;
    let bottom_end = bottom_start + clipped.w as usize;
    pixels[bottom_start..bottom_end].fill(BORDER_BGRA_PREMULT);

    for y in (clipped.y + 1)..(clipped.y + clipped.h - 1) {
        let row_start = (y as usize) * pitch;
        pixels[row_start + left] = BORDER_BGRA_PREMULT;
        pixels[row_start + right] = BORDER_BGRA_PREMULT;
    }

    Ok(())
}

fn clip_to_desktop(context: &DragOverlayContext, rect: DragRect) -> Option<ClippedRect> {
    if rect.w <= 0 || rect.h <= 0 {
        return None;
    }

    let left = rect.x.max(context.desktop_x);
    let top = rect.y.max(context.desktop_y);
    let right = (rect.x + rect.w).min(context.desktop_x + context.desktop_w);
    let bottom = (rect.y + rect.h).min(context.desktop_y + context.desktop_h);

    if right <= left || bottom <= top {
        return None;
    }

    Some(ClippedRect {
        x: left - context.desktop_x,
        y: top - context.desktop_y,
        w: right - left,
        h: bottom - top,
    })
}

fn update_drag_state(context: &mut DragOverlayContext, hwnd: HWND, message: u32, lparam: LPARAM) {
    let client_x = (lparam.0 & 0xFFFF) as u16 as i16 as i32;
    let client_y = ((lparam.0 >> 16) & 0xFFFF) as u16 as i16 as i32;
    let screen_x = context.desktop_x + client_x;
    let screen_y = context.desktop_y + client_y;

    match message {
        WM_LBUTTONDOWN => {
            if matches!(context.state, DragState::Waiting) {
                context.state = DragState::Dragging {
                    start: (screen_x, screen_y),
                    current: (screen_x, screen_y),
                };
                unsafe {
                    let _ = SetCapture(hwnd);
                }
                post_render_if_needed(context);
                eprintln!("[drag_overlay] drag start=({}, {})", screen_x, screen_y);
            }
        }
        WM_MOUSEMOVE => {
            if let DragState::Dragging { current, .. } = &mut context.state {
                *current = (screen_x, screen_y);
                post_render_if_needed(context);
            }
        }
        WM_LBUTTONUP => {
            let previous_state = std::mem::replace(&mut context.state, DragState::Idle);
            if let DragState::Dragging { start, .. } = previous_state {
                if unsafe { GetCapture() } == hwnd {
                    unsafe {
                        let _ = ReleaseCapture();
                    }
                }

                let rect = normalize_rect(start, (screen_x, screen_y));
                eprintln!(
                    "[drag_overlay] drag end rect x={} y={} w={} h={}",
                    rect.x, rect.y, rect.w, rect.h
                );

                if rect.w > 3 && rect.h > 3 {
                    capture::try_enqueue_request(CaptureRequest::SelectedRect {
                        rect: CaptureScreenRect {
                            x: rect.x,
                            y: rect.y,
                            w: rect.w,
                            h: rect.h,
                        },
                        queued_at: Instant::now(),
                    });
                    eprintln!("[drag_overlay] rect accepted and enqueued");
                } else {
                    eprintln!("[drag_overlay] rect too small, cancel");
                }

                if let Err(err) = hide_and_reset_overlay(context) {
                    eprintln!("[drag_overlay] hide after button up failed: {err}");
                }
            } else {
                context.state = previous_state;
            }
        }
        _ => {}
    }
}

fn normalize_rect(start: (i32, i32), end: (i32, i32)) -> DragRect {
    let left = start.0.min(end.0);
    let top = start.1.min(end.1);
    let right = start.0.max(end.0);
    let bottom = start.1.max(end.1);
    DragRect {
        x: left,
        y: top,
        w: right - left,
        h: bottom - top,
    }
}

extern "system" fn drag_overlay_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_SETCURSOR => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut DragOverlayContext;
            if !ptr.is_null() {
                let context = unsafe { &mut *ptr };
                if !matches!(context.state, DragState::Idle) {
                    if let Ok(cursor) = unsafe { LoadCursorW(None, IDC_CROSS) } {
                        unsafe {
                            SetCursor(cursor);
                        }
                        return LRESULT(1);
                    }
                }
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_DISPLAYCHANGE => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut DragOverlayContext;
            if !ptr.is_null() {
                let context = unsafe { &mut *ptr };
                context.pending_resize = true;
                if matches!(context.state, DragState::Dragging { .. }) {
                    if let Err(err) = hide_and_reset_overlay(context) {
                        eprintln!("[drag_overlay] hide on display change failed: {err}");
                    }
                }
            }
            LRESULT(0)
        }
        WM_APP_RENDER => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut DragOverlayContext;
            if !ptr.is_null() {
                let context = unsafe { &mut *ptr };
                if let Err(err) = render_current_state(context) {
                    eprintln!("[drag_overlay] render failed: {err}");
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN | WM_MOUSEMOVE | WM_LBUTTONUP => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut DragOverlayContext;
            if !ptr.is_null() {
                let context = unsafe { &mut *ptr };
                update_drag_state(context, hwnd, msg, lparam);
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
