use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, PostThreadMessageW, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK,
    MSG, WH_MOUSE_LL, WM_LBUTTONDOWN, WM_QUIT, WM_RBUTTONDOWN, WM_RBUTTONUP,
};

const EVENT_CHANNEL_CAPACITY: usize = 1024;
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

static MOUSE_HOOK_RUNTIME: OnceLock<MouseHookRuntime> = OnceLock::new();
static MOUSE_EVENT_TX: OnceLock<mpsc::SyncSender<MouseEvent>> = OnceLock::new();
static RIGHT_MOUSE_BUTTON_HELD: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy)]
pub enum MouseEvent {
    LeftDown,
    RightDown,
    RightUp,
}

struct MouseHookRuntime {
    state: Mutex<Option<MouseHookState>>,
    rx: Mutex<mpsc::Receiver<MouseEvent>>,
    active: AtomicBool,
}

struct MouseHookState {
    thread_id: u32,
    join: JoinHandle<()>,
}

pub fn install() -> io::Result<()> {
    let runtime = runtime();
    if runtime.active.load(Ordering::Acquire) {
        return Ok(());
    }

    let mut state_guard = runtime
        .state
        .lock()
        .map_err(|_| io::Error::other("mouse hook state lock poisoned"))?;
    if state_guard.is_some() {
        runtime.active.store(true, Ordering::Release);
        return Ok(());
    }

    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let join = thread::Builder::new()
        .name("mouse-hook".to_string())
        .spawn(move || {
            let hook_result = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(hook_proc), None, 0) };

            match hook_result {
                Ok(hook) => {
                    let thread_id = unsafe { GetCurrentThreadId() };
                    let _ = ready_tx.send(Ok(thread_id));
                    unsafe { message_loop(hook) };
                }
                Err(err) => {
                    let _ = ready_tx.send(Err(io::Error::other(format!(
                        "SetWindowsHookExW(WH_MOUSE_LL) failed: {err}"
                    ))));
                }
            }

            RIGHT_MOUSE_BUTTON_HELD.store(false, Ordering::Relaxed);
        })?;

    let thread_id = ready_rx.recv().unwrap_or_else(|_| {
        Err(io::Error::other(
            "mouse hook thread exited before initialization",
        ))
    })?;

    *state_guard = Some(MouseHookState { thread_id, join });
    runtime.active.store(true, Ordering::Release);
    Ok(())
}

pub fn uninstall() {
    let Some(runtime) = MOUSE_HOOK_RUNTIME.get() else {
        return;
    };

    runtime.active.store(false, Ordering::Release);
    RIGHT_MOUSE_BUTTON_HELD.store(false, Ordering::Relaxed);

    let state = runtime
        .state
        .lock()
        .ok()
        .and_then(|mut guard| guard.take());
    let Some(state) = state else {
        return;
    };

    let _ = unsafe { PostThreadMessageW(state.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };

    let (done_tx, done_rx) = mpsc::sync_channel(1);
    let _ = thread::Builder::new()
        .name("mouse-hook-join-wait".to_string())
        .spawn(move || {
            let _ = state.join.join();
            let _ = done_tx.send(());
        });
    let _ = done_rx.recv_timeout(SHUTDOWN_TIMEOUT);
}

pub fn is_active() -> bool {
    MOUSE_HOOK_RUNTIME
        .get()
        .map(|runtime| runtime.active.load(Ordering::Acquire))
        .unwrap_or(false)
}

pub fn right_button_held() -> bool {
    RIGHT_MOUSE_BUTTON_HELD.load(Ordering::Relaxed)
}

pub fn try_recv_event() -> Option<MouseEvent> {
    let runtime = MOUSE_HOOK_RUNTIME.get()?;
    let rx_guard = runtime.rx.lock().ok()?;
    rx_guard.try_recv().ok()
}

fn runtime() -> &'static MouseHookRuntime {
    MOUSE_HOOK_RUNTIME.get_or_init(|| {
        let (tx, rx) = mpsc::sync_channel(EVENT_CHANNEL_CAPACITY);
        let _ = MOUSE_EVENT_TX.set(tx);
        MouseHookRuntime {
            state: Mutex::new(None),
            rx: Mutex::new(rx),
            active: AtomicBool::new(false),
        }
    })
}

unsafe fn message_loop(hook: HHOOK) {
    let mut msg = MSG::default();

    loop {
        let result = unsafe { GetMessageW(&mut msg, None, 0, 0) }.0;
        if result <= 0 {
            break;
        }
    }

    let _ = unsafe { UnhookWindowsHookEx(hook) };
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }

    let message = wparam.0 as u32;
    let event = match message {
        WM_LBUTTONDOWN => Some(MouseEvent::LeftDown),
        WM_RBUTTONDOWN => {
            RIGHT_MOUSE_BUTTON_HELD.store(true, Ordering::Relaxed);
            Some(MouseEvent::RightDown)
        }
        WM_RBUTTONUP => {
            RIGHT_MOUSE_BUTTON_HELD.store(false, Ordering::Relaxed);
            Some(MouseEvent::RightUp)
        }
        _ => None,
    };

    if let Some(event) = event {
        if let Some(tx) = MOUSE_EVENT_TX.get() {
            let _ = tx.try_send(event);
        }
        return LRESULT(1);
    }

    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}
