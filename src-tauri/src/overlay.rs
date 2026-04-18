use std::io;
use std::ptr;
use std::ffi::c_void;
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
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, CreateCompatibleDC,
    CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, HBITMAP, HDC, HGDIOBJ, SelectObject,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, KillTimer,
    PostMessageW, PostQuitMessage, RegisterClassW, SW_HIDE, SW_SHOWNOACTIVATE, SET_WINDOW_POS_FLAGS,
    SetTimer, SetWindowPos, ShowWindow, TranslateMessage, UpdateLayeredWindow, CS_HREDRAW, CS_VREDRAW,
    HMENU, MSG, SWP_NOACTIVATE, SWP_SHOWWINDOW, ULW_ALPHA, UPDATE_LAYERED_WINDOW_FLAGS,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_DESTROY, WM_TIMER, WNDCLASSW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP, HWND_TOPMOST,
};

use crate::capture::pipeline::BoundingBoxScreen;

const OVERLAY_CHANNEL_CAPACITY: usize = 16;
const OVERLAY_INIT_TIMEOUT: Duration = Duration::from_secs(3);
const OVERLAY_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
const OVERLAY_HIDE_MS: u32 = 500;
const OVERLAY_TIMER_ID: usize = 1;
const WM_APP_CMD: u32 = WM_APP + 1;

static OVERLAY_RUNTIME: OnceLock<OverlayRuntime> = OnceLock::new();
static OVERLAY_HWND_RAW: AtomicIsize = AtomicIsize::new(0);
static WARNED_NOT_INITIALIZED: AtomicBool = AtomicBool::new(false);

struct OverlayRuntime {
    tx: SyncSender<OverlayCommand>,
    join: Mutex<Option<JoinHandle<()>>>,
}

enum OverlayCommand {
    Show(BoundingBoxScreen),
    Hide,
    Exit,
}

struct OverlayContext {
    hwnd: HWND,
    hdc_mem: HDC,
    hbm: HBITMAP,
    old_bitmap: HGDIOBJ,
    bits_ptr: *mut u8,
    cur_w: i32,
    cur_h: i32,
}

pub fn init() -> io::Result<()> {
    if OVERLAY_RUNTIME.get().is_some() {
        return Ok(());
    }

    let (cmd_tx, cmd_rx) = sync_channel(OVERLAY_CHANNEL_CAPACITY);
    let (ready_tx, ready_rx) = sync_channel(0);

    let join = thread::Builder::new()
        .name("overlay-thread".to_string())
        .spawn(move || overlay_thread_main(cmd_rx, ready_tx))
        .map_err(|err| io::Error::other(format!("spawn overlay thread failed: {err}")))?;

    let ready_result = ready_rx
        .recv_timeout(OVERLAY_INIT_TIMEOUT)
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "overlay init handshake timed out"))?;

    if let Err(err) = ready_result {
        let _ = join.join();
        return Err(err);
    }

    OVERLAY_RUNTIME
        .set(OverlayRuntime {
            tx: cmd_tx,
            join: Mutex::new(Some(join)),
        })
        .map_err(|_| io::Error::new(io::ErrorKind::AlreadyExists, "overlay runtime already initialized"))?;

    Ok(())
}

pub fn show(bbox: BoundingBoxScreen) {
    if bbox.w <= 0 || bbox.h <= 0 {
        return;
    }

    let Some(runtime) = OVERLAY_RUNTIME.get() else {
        if !WARNED_NOT_INITIALIZED.swap(true, Ordering::Relaxed) {
            eprintln!("[overlay] overlay not initialized");
        }
        return;
    };

    let _ = runtime.tx.try_send(OverlayCommand::Show(bbox));
    wake_overlay_thread();
}

pub fn shutdown() {
    let Some(runtime) = OVERLAY_RUNTIME.get() else {
        return;
    };

    let _ = runtime.tx.try_send(OverlayCommand::Hide);
    let _ = runtime.tx.try_send(OverlayCommand::Exit);
    wake_overlay_thread();

    let join = runtime.join.lock().ok().and_then(|mut guard| guard.take());
    if let Some(join) = join {
        let (done_tx, done_rx) = sync_channel(1);
        let _ = thread::Builder::new()
            .name("overlay-join-wait".to_string())
            .spawn(move || {
                let _ = join.join();
                let _ = done_tx.send(());
            });

        let _ = done_rx.recv_timeout(OVERLAY_SHUTDOWN_TIMEOUT);
    }
}

fn overlay_thread_main(
    rx: Receiver<OverlayCommand>,
    ready_tx: SyncSender<io::Result<()>>,
) {
    if let Err(err) = run_overlay_thread(rx, &ready_tx) {
        let _ = ready_tx.send(Err(err));
    }
}

fn run_overlay_thread(
    rx: Receiver<OverlayCommand>,
    ready_tx: &SyncSender<io::Result<()>>,
) -> io::Result<()> {
    let hinstance = unsafe { GetModuleHandleW(None) }
        .map(|hmodule| HINSTANCE(hmodule.0))
        .map_err(|err| io::Error::other(format!("GetModuleHandleW failed: {err}")))?;

    register_overlay_class(hinstance)?;
    let hwnd = create_overlay_window(hinstance)?;

    let hdc_mem = unsafe { CreateCompatibleDC(None) };
    if hdc_mem.0.is_null() {
        return Err(io::Error::other("CreateCompatibleDC failed"));
    }

    let mut context = OverlayContext {
        hwnd,
        hdc_mem,
        hbm: HBITMAP::default(),
        old_bitmap: HGDIOBJ::default(),
        bits_ptr: ptr::null_mut(),
        cur_w: 0,
        cur_h: 0,
    };

    OVERLAY_HWND_RAW.store(hwnd.0 as isize, Ordering::Relaxed);
    let _ = ready_tx.send(Ok(()));

    let mut msg = MSG::default();
    let mut exiting = false;
    loop {
        let status = unsafe { GetMessageW(&mut msg, None, 0, 0) }.0;
        if status == -1 {
            break;
        }
        if status == 0 {
            break;
        }

        if msg.message == WM_APP_CMD {
            if !drain_commands(&rx, &mut context, &mut exiting) {
                break;
            }
            continue;
        }

        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    cleanup_gdi(&mut context);
    OVERLAY_HWND_RAW.store(0, Ordering::Relaxed);
    Ok(())
}

fn drain_commands(
    rx: &Receiver<OverlayCommand>,
    context: &mut OverlayContext,
    exiting: &mut bool,
) -> bool {
    while let Ok(command) = rx.try_recv() {
        match command {
            OverlayCommand::Show(bbox) => {
                if let Err(err) = handle_show(context, bbox) {
                    eprintln!("[overlay] show command failed: {err}");
                }
            }
            OverlayCommand::Hide => {
                hide_window(context.hwnd);
            }
            OverlayCommand::Exit => {
                *exiting = true;
                hide_window(context.hwnd);
                cleanup_gdi(context);
                let _ = unsafe { DestroyWindow(context.hwnd) };
                return false;
            }
        }
    }
    !*exiting
}

fn handle_show(context: &mut OverlayContext, bbox: BoundingBoxScreen) -> io::Result<()> {
    let target_w = bbox.w + 2;
    let target_h = bbox.h + 2;
    if target_w <= 0 || target_h <= 0 {
        return Ok(());
    }

    if target_w != context.cur_w || target_h != context.cur_h {
        recreate_dib(context, target_w, target_h)?;
    }

    fill_stub_dib(context)?;

    let pos_flags = SET_WINDOW_POS_FLAGS(SWP_NOACTIVATE.0 | SWP_SHOWWINDOW.0);
    unsafe {
        SetWindowPos(
            context.hwnd,
            HWND_TOPMOST,
            bbox.x - 1,
            bbox.y - 1,
            target_w,
            target_h,
            pos_flags,
        )
    }
    .map_err(|err| io::Error::other(format!("SetWindowPos failed: {err}")))?;

    let size = SIZE {
        cx: target_w,
        cy: target_h,
    };
    let src_point = POINT { x: 0, y: 0 };
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
            None,
            Some(&size),
            context.hdc_mem,
            Some(&src_point),
            COLORREF(0),
            Some(&blend),
            UPDATE_LAYERED_WINDOW_FLAGS(ULW_ALPHA.0),
        )
    }
    .map_err(|err| io::Error::other(format!("UpdateLayeredWindow failed: {err}")))?;

    unsafe {
        let _ = ShowWindow(context.hwnd, SW_SHOWNOACTIVATE);
    }
    unsafe {
        let _ = KillTimer(context.hwnd, OVERLAY_TIMER_ID);
    }
    let timer_id = unsafe { SetTimer(context.hwnd, OVERLAY_TIMER_ID, OVERLAY_HIDE_MS, None) };
    if timer_id == 0 {
        return Err(io::Error::other("SetTimer failed"));
    }

    Ok(())
}

fn recreate_dib(context: &mut OverlayContext, width: i32, height: i32) -> io::Result<()> {
    if !context.hbm.0.is_null() {
        unsafe {
            let _ = SelectObject(context.hdc_mem, context.old_bitmap);
            let _ = DeleteObject(context.hbm);
        }
        context.hbm = HBITMAP::default();
        context.bits_ptr = ptr::null_mut();
    }

    let mut bmi = BITMAPINFO::default();
    bmi.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: width,
        biHeight: -height,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };

    let mut bits: *mut core::ffi::c_void = ptr::null_mut();
    let hbm = unsafe {
        CreateDIBSection(
            context.hdc_mem,
            &bmi,
            DIB_RGB_COLORS,
            &mut bits,
            None,
            0,
        )
    }
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
    context.cur_w = width;
    context.cur_h = height;

    Ok(())
}

fn fill_stub_dib(context: &OverlayContext) -> io::Result<()> {
    if context.bits_ptr.is_null() || context.cur_w <= 0 || context.cur_h <= 0 {
        return Err(io::Error::other("overlay dib is not initialized"));
    }

    let len = (context.cur_w as usize) * (context.cur_h as usize);
    let pixel = u32::from_le_bytes([0xC0, 0x60, 0x00, 0xC0]);
    let pixels = unsafe { std::slice::from_raw_parts_mut(context.bits_ptr as *mut u32, len) };
    pixels.fill(pixel);
    Ok(())
}

fn cleanup_gdi(context: &mut OverlayContext) {
    unsafe {
        let _ = KillTimer(context.hwnd, OVERLAY_TIMER_ID);
    }

    if !context.hdc_mem.0.is_null() {
        unsafe {
            if !context.hbm.0.is_null() && !context.old_bitmap.0.is_null() {
                let _ = SelectObject(context.hdc_mem, context.old_bitmap);
            }
            if !context.hbm.0.is_null() {
                let _ = DeleteObject(context.hbm);
            }
            let _ = DeleteDC(context.hdc_mem);
        }
    }

    context.hbm = HBITMAP::default();
    context.old_bitmap = HGDIOBJ::default();
    context.hdc_mem = HDC::default();
    context.bits_ptr = ptr::null_mut();
    context.cur_w = 0;
    context.cur_h = 0;
}

fn register_overlay_class(hinstance: HINSTANCE) -> io::Result<()> {
    let window_class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(overlay_wnd_proc),
        hInstance: hinstance,
        lpszClassName: w!("Capture2TextOverlayWindow"),
        ..Default::default()
    };

    let atom = unsafe { RegisterClassW(&window_class) };
    if atom == 0 {
        let err = unsafe { GetLastError() };
        if err != ERROR_CLASS_ALREADY_EXISTS {
            return Err(io::Error::other(format!("RegisterClassW failed: {err:?}")));
        }
    }

    Ok(())
}

fn create_overlay_window(hinstance: HINSTANCE) -> io::Result<HWND> {
    let ex_style = WINDOW_EX_STYLE(
        WS_EX_LAYERED.0
            | WS_EX_TRANSPARENT.0
            | WS_EX_TOPMOST.0
            | WS_EX_TOOLWINDOW.0
            | WS_EX_NOACTIVATE.0,
    );
    let style = WINDOW_STYLE(WS_POPUP.0);

    let hwnd = unsafe {
        CreateWindowExW(
            ex_style,
            w!("Capture2TextOverlayWindow"),
            w!("Capture2TextOverlayWindow"),
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
    .map_err(|err| io::Error::other(format!("CreateWindowExW failed: {err}")))?;

    Ok(hwnd)
}

fn hide_window(hwnd: HWND) {
    unsafe {
        let _ = KillTimer(hwnd, OVERLAY_TIMER_ID);
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
}

fn wake_overlay_thread() {
    let hwnd_raw = OVERLAY_HWND_RAW.load(Ordering::Relaxed);
    if hwnd_raw == 0 {
        return;
    }

    let hwnd = HWND(hwnd_raw as *mut c_void);
    let _ = unsafe { PostMessageW(hwnd, WM_APP_CMD, WPARAM(0), LPARAM(0)) };
}

extern "system" fn overlay_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TIMER => {
            hide_window(hwnd);
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
