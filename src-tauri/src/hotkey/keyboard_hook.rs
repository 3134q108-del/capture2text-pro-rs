use std::io;
use std::sync::{mpsc, Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Instant;
use std::{env, fmt};
use std::time::Duration;

use crate::capture::{self, CursorPoint, HotkeyKind};
use crate::drag_overlay;

use windows::Win32::Foundation::{LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    KEYEVENTF_KEYUP, VK_CONTROL, VK_F18, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetCursorPos, GetMessageW, GetPhysicalCursorPos, PostThreadMessageW,
    SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_KEYDOWN,
    WM_QUIT, WM_SYSKEYDOWN,
};

const VK_Q: u32 = 0x51;
const VK_W: u32 = 0x57;
const VK_E: u32 = 0x45;
const VK_ESCAPE: u32 = 0x1B;
const HOTKEY_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

static HOTKEY_RUNTIME: OnceLock<HotkeyRuntime> = OnceLock::new();
static HOTKEY_TRACE_ENABLED: AtomicBool = AtomicBool::new(false);

struct HotkeyRuntime {
    thread_id: u32,
    join: Mutex<Option<JoinHandle<()>>>,
    trace_tx: Mutex<Option<mpsc::SyncSender<TraceEvent>>>,
    trace_join: Mutex<Option<JoinHandle<()>>>,
}

#[derive(Clone, Copy)]
enum TraceKind {
    Q,
    W,
    E,
}

impl fmt::Display for TraceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Q => write!(f, "Q"),
            Self::W => write!(f, "W"),
            Self::E => write!(f, "E"),
        }
    }
}

#[derive(Clone, Copy)]
enum TraceEvent {
    Consumed(TraceKind),
}

pub fn install() -> io::Result<()> {
    if HOTKEY_RUNTIME.get().is_some() {
        return Ok(());
    }

    let trace_enabled = matches!(env::var("C2T_HOTKEY_TRACE").ok().as_deref(), Some("1"));
    HOTKEY_TRACE_ENABLED.store(trace_enabled, Ordering::Relaxed);

    let (trace_tx, trace_join) = if trace_enabled {
        let (tx, rx) = mpsc::sync_channel(1024);
        let join = thread::Builder::new()
            .name("hotkey-trace".to_string())
            .spawn(move || {
                for event in rx {
                    match event {
                        TraceEvent::Consumed(kind) => {
                            eprintln!(
                                "[hotkey] Win+{} detected kind={} consumed=LRESULT(1)",
                                kind, kind
                            );
                        }
                    }
                }
            })
            .map_err(|err| io::Error::other(format!("failed to spawn hotkey trace thread: {err}")))?;
        (Some(tx), Some(join))
    } else {
        (None, None)
    };

    let (ready_tx, ready_rx) = mpsc::sync_channel(1);

    let join = thread::Builder::new()
        .name("keyboard-hook".to_string())
        .spawn(move || {
            let hook_result = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), None, 0) };

            match hook_result {
                Ok(hook) => {
                    let thread_id = unsafe { GetCurrentThreadId() };
                    println!(
                        "[hotkey] WH_KEYBOARD_LL installed, thread id={}",
                        thread_id
                    );
                    let _ = ready_tx.send(Ok(thread_id));
                    unsafe { message_loop(hook) };
                }
                Err(err) => {
                    let _ = ready_tx.send(Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("SetWindowsHookExW failed: {err}"),
                    )));
                }
            }
        })?;

    let thread_id = ready_rx.recv().unwrap_or_else(|_| {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "keyboard hook thread exited before initialization",
        ))
    })?;

    HOTKEY_RUNTIME
        .set(HotkeyRuntime {
            thread_id,
            join: Mutex::new(Some(join)),
            trace_tx: Mutex::new(trace_tx),
            trace_join: Mutex::new(trace_join),
        })
        .map_err(|_| io::Error::new(io::ErrorKind::AlreadyExists, "hotkey runtime already initialized"))?;

    Ok(())
}

pub fn shutdown() {
    let Some(runtime) = HOTKEY_RUNTIME.get() else {
        return;
    };

    let _ = unsafe { PostThreadMessageW(runtime.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };

    let join = runtime.join.lock().ok().and_then(|mut guard| guard.take());
    if let Some(join) = join {
        let (done_tx, done_rx) = mpsc::sync_channel(1);
        let _ = thread::Builder::new()
            .name("keyboard-hook-join-wait".to_string())
            .spawn(move || {
                let _ = join.join();
                let _ = done_tx.send(());
            });
        let _ = done_rx.recv_timeout(HOTKEY_SHUTDOWN_TIMEOUT);
    }

    let _ = runtime.trace_tx.lock().ok().and_then(|mut guard| guard.take());
    let trace_join = runtime.trace_join.lock().ok().and_then(|mut guard| guard.take());
    if let Some(trace_join) = trace_join {
        let _ = trace_join.join();
    }
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
    if message == WM_KEYDOWN || message == WM_SYSKEYDOWN {
        let kbd = unsafe { *(lparam.0 as *const KBDLLHOOKSTRUCT) };
        let vk = kbd.vkCode;

        let ctrl_down = key_down(i32::from(VK_CONTROL.0));
        let shift_down = key_down(i32::from(VK_SHIFT.0));
        let win_down = key_down(i32::from(VK_LWIN.0)) || key_down(i32::from(VK_RWIN.0));
        let alt_down = key_down(i32::from(VK_MENU.0));

        let no_modifiers = !win_down && !ctrl_down && !shift_down && !alt_down;
        if vk == VK_ESCAPE && no_modifiers {
            if drag_overlay::is_active() {
                drag_overlay::cancel();
                return LRESULT(1);
            }
            return unsafe { CallNextHookEx(None, code, wparam, lparam) };
        }

        let is_target = matches!(vk, VK_Q | VK_W | VK_E);
        if is_target && win_down && !ctrl_down && !shift_down && !alt_down {
            unsafe { send_ctrl_tap() };

            match vk {
                VK_Q => {
                    trace_consumed(TraceKind::Q);
                    let _reserved_q = HotkeyKind::Q;
                    if drag_overlay::is_active() {
                        drag_overlay::cancel();
                    } else {
                        drag_overlay::begin_drag();
                    }
                    return LRESULT(1);
                }
                VK_W | VK_E => {
                    if drag_overlay::is_active() {
                        if vk == VK_W {
                            trace_consumed(TraceKind::W);
                        } else {
                            trace_consumed(TraceKind::E);
                        }
                        return LRESULT(1);
                    }

                    let kind = if vk == VK_W {
                        trace_consumed(TraceKind::W);
                        HotkeyKind::W
                    } else {
                        trace_consumed(TraceKind::E);
                        HotkeyKind::E
                    };

                    if let Some(cursor) = read_cursor_point() {
                        capture::try_enqueue_from_hook(kind, cursor, Instant::now());
                    }
                    return LRESULT(1);
                }
                _ => {}
            }
        }
    }

    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn trace_consumed(kind: TraceKind) {
    if !HOTKEY_TRACE_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let Some(runtime) = HOTKEY_RUNTIME.get() else {
        return;
    };
    let Some(tx) = runtime.trace_tx.lock().ok().and_then(|guard| guard.clone()) else {
        return;
    };
    let _ = tx.try_send(TraceEvent::Consumed(kind));
}

fn key_down(vk: i32) -> bool {
    (unsafe { GetKeyState(vk) } as u16 & 0x8000) != 0
}

fn read_cursor_point() -> Option<CursorPoint> {
    let mut point = POINT::default();

    if unsafe { GetPhysicalCursorPos(&mut point) }.is_ok() {
        return Some(CursorPoint {
            x: point.x,
            y: point.y,
        });
    }

    if unsafe { GetCursorPos(&mut point) }.is_ok() {
        return Some(CursorPoint {
            x: point.x,
            y: point.y,
        });
    }

    None
}

unsafe fn send_ctrl_tap() {
    // Use VK_F18 (not VK_CONTROL) to avoid IME switching the input language:
    // Chinese IME on Win11 intercepts Ctrl taps and toggles EN/CH mode.
    // Any unused key (F13-F24) works; F18 picked per MS virtual-key docs.
    let inputs = [
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_F18,
                    wScan: 0,
                    dwFlags: KEYBD_EVENT_FLAGS(0),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_F18,
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
    ];

    let _ = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
}
