use std::io;
use std::sync::{mpsc, Mutex, OnceLock, RwLock};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Instant;
use std::{env, fmt};
use std::time::Duration;

use crate::capture::{self, CursorPoint, HotkeyKind};
use crate::drag_overlay;

use windows::Win32::Foundation::{LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_RCONTROL, VK_RMENU,
    VK_RSHIFT, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetCursorPos, GetMessageW, GetPhysicalCursorPos, PostThreadMessageW,
    SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG,
    WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

const VK_ESCAPE: u32 = 0x1B;
const HOTKEY_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

static HOTKEY_RUNTIME: OnceLock<HotkeyRuntime> = OnceLock::new();
static HOTKEY_TRACE_ENABLED: AtomicBool = AtomicBool::new(false);
static MODIFIER_STATE: AtomicU8 = AtomicU8::new(0);

const MOD_CTRL: u8 = 1 << 0;
const MOD_SHIFT: u8 = 1 << 1;
const MOD_ALT: u8 = 1 << 2;
const MOD_WIN: u8 = 1 << 3;

struct HotkeyRuntime {
    thread_id: u32,
    join: Mutex<Option<JoinHandle<()>>>,
    trace_tx: Mutex<Option<mpsc::SyncSender<TraceEvent>>>,
    trace_join: Mutex<Option<JoinHandle<()>>>,
}

#[derive(Clone, Copy)]
pub struct HotkeyConfig {
    pub q: crate::window_state::HotkeyBinding,
    pub w: crate::window_state::HotkeyBinding,
    pub e: crate::window_state::HotkeyBinding,
}

#[derive(Clone, Copy)]
enum TraceEvent {
    Consumed(HotkeyKind),
}

static HOTKEY_CONFIG: OnceLock<RwLock<HotkeyConfig>> = OnceLock::new();

pub fn install() -> io::Result<()> {
    if HOTKEY_RUNTIME.get().is_some() {
        return Ok(());
    }

    MODIFIER_STATE.store(0, Ordering::Relaxed);

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
                                "[hotkey] consumed kind={} combo={}",
                                kind,
                                format_binding(binding_for_kind(kind))
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

pub fn config() -> HotkeyConfig {
    HOTKEY_CONFIG
        .get_or_init(|| RwLock::new(load_config_from_state()))
        .read()
        .map(|guard| *guard)
        .unwrap_or_else(|_| load_config_from_state())
}

pub fn set_config(next: HotkeyConfig) {
    if let Ok(mut guard) = HOTKEY_CONFIG
        .get_or_init(|| RwLock::new(load_config_from_state()))
        .write()
    {
        *guard = next;
    }
}

pub fn reload_from_state() {
    set_config(load_config_from_state());
}

pub fn default_config() -> HotkeyConfig {
    HotkeyConfig {
        q: crate::window_state::HotkeyBinding {
            modifiers: crate::window_state::HotkeyModifiers {
                win: true,
                ..crate::window_state::HotkeyModifiers::default()
            },
            key_code: 0x51,
        },
        w: crate::window_state::HotkeyBinding {
            modifiers: crate::window_state::HotkeyModifiers {
                win: true,
                ..crate::window_state::HotkeyModifiers::default()
            },
            key_code: 0x57,
        },
        e: crate::window_state::HotkeyBinding {
            modifiers: crate::window_state::HotkeyModifiers {
                win: true,
                ..crate::window_state::HotkeyModifiers::default()
            },
            key_code: 0x45,
        },
    }
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
    let kbd = unsafe { *(lparam.0 as *const KBDLLHOOKSTRUCT) };
    let vk = kbd.vkCode;
    let injected = (kbd.flags.0 & LLKHF_INJECTED.0) != 0;

    if !injected {
        if let Some(bit) = vk_to_modifier_bit(vk) {
            match message {
                WM_KEYDOWN | WM_SYSKEYDOWN => set_modifier(bit, true),
                WM_KEYUP | WM_SYSKEYUP => set_modifier(bit, false),
                _ => {}
            }
        }
    }

    if message == WM_KEYDOWN || message == WM_SYSKEYDOWN {
        let no_modifiers = MODIFIER_STATE.load(Ordering::Relaxed) == 0;
        if vk == VK_ESCAPE && no_modifiers {
            if drag_overlay::is_active() {
                drag_overlay::cancel();
                return LRESULT(1);
            }
            return unsafe { CallNextHookEx(None, code, wparam, lparam) };
        }

        let modifiers = current_modifiers();
        let cfg = config();

        if let Some(kind) = matched_kind(vk, modifiers, cfg) {
            if !injected {
                unsafe { send_ctrl_tap() };
            }
            match kind {
                HotkeyKind::Q => {
                    trace_consumed(HotkeyKind::Q);
                    if drag_overlay::is_active() {
                        drag_overlay::cancel();
                    } else {
                        drag_overlay::begin_drag();
                    }
                }
                HotkeyKind::W | HotkeyKind::E => {
                    trace_consumed(kind);
                    if drag_overlay::is_active() {
                        drag_overlay::cancel();
                    }
                    if let Some(cursor) = read_cursor_point() {
                        capture::try_enqueue_from_hook(kind, cursor, Instant::now());
                    }
                }
            }
            return LRESULT(1);
        }
    }

    if message == WM_KEYUP || message == WM_SYSKEYUP {
        if matched_kind(vk, current_modifiers(), config()).is_some() {
            return LRESULT(1);
        }
    }

    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn load_config_from_state() -> HotkeyConfig {
    HotkeyConfig {
        q: crate::window_state::hotkey_q(),
        w: crate::window_state::hotkey_w(),
        e: crate::window_state::hotkey_e(),
    }
}

fn matched_kind(
    vk: u32,
    modifiers: crate::window_state::HotkeyModifiers,
    cfg: HotkeyConfig,
) -> Option<HotkeyKind> {
    if binding_matches(vk, modifiers, cfg.q) {
        return Some(HotkeyKind::Q);
    }
    if binding_matches(vk, modifiers, cfg.w) {
        return Some(HotkeyKind::W);
    }
    if binding_matches(vk, modifiers, cfg.e) {
        return Some(HotkeyKind::E);
    }
    None
}

fn binding_matches(
    vk: u32,
    modifiers: crate::window_state::HotkeyModifiers,
    binding: crate::window_state::HotkeyBinding,
) -> bool {
    vk == binding.key_code
        && modifiers.ctrl == binding.modifiers.ctrl
        && modifiers.shift == binding.modifiers.shift
        && modifiers.alt == binding.modifiers.alt
        && modifiers.win == binding.modifiers.win
}

fn trace_consumed(kind: HotkeyKind) {
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

fn binding_for_kind(kind: HotkeyKind) -> crate::window_state::HotkeyBinding {
    match kind {
        HotkeyKind::Q => config().q,
        HotkeyKind::W => config().w,
        HotkeyKind::E => config().e,
    }
}

fn format_binding(binding: crate::window_state::HotkeyBinding) -> String {
    let mut parts: Vec<String> = Vec::new();
    if binding.modifiers.ctrl {
        parts.push("Ctrl".to_string());
    }
    if binding.modifiers.shift {
        parts.push("Shift".to_string());
    }
    if binding.modifiers.alt {
        parts.push("Alt".to_string());
    }
    if binding.modifiers.win {
        parts.push("Win".to_string());
    }
    parts.push(format!("VK_{:X}", binding.key_code));
    parts.join("+")
}

impl fmt::Display for HotkeyKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HotkeyKind::Q => write!(f, "Q"),
            HotkeyKind::W => write!(f, "W"),
            HotkeyKind::E => write!(f, "E"),
        }
    }
}

fn set_modifier(bit: u8, on: bool) {
    if on {
        MODIFIER_STATE.fetch_or(bit, Ordering::Relaxed);
    } else {
        MODIFIER_STATE.fetch_and(!bit, Ordering::Relaxed);
    }
}

fn current_modifiers() -> crate::window_state::HotkeyModifiers {
    let state = MODIFIER_STATE.load(Ordering::Relaxed);
    crate::window_state::HotkeyModifiers {
        ctrl: (state & MOD_CTRL) != 0,
        shift: (state & MOD_SHIFT) != 0,
        alt: (state & MOD_ALT) != 0,
        win: (state & MOD_WIN) != 0,
    }
}

fn vk_to_modifier_bit(vk: u32) -> Option<u8> {
    if vk == VK_CONTROL.0 as u32 || vk == VK_LCONTROL.0 as u32 || vk == VK_RCONTROL.0 as u32 {
        return Some(MOD_CTRL);
    }
    if vk == VK_SHIFT.0 as u32 || vk == VK_LSHIFT.0 as u32 || vk == VK_RSHIFT.0 as u32 {
        return Some(MOD_SHIFT);
    }
    if vk == VK_MENU.0 as u32 || vk == VK_LMENU.0 as u32 || vk == VK_RMENU.0 as u32 {
        return Some(MOD_ALT);
    }
    if vk == VK_LWIN.0 as u32 || vk == VK_RWIN.0 as u32 {
        return Some(MOD_WIN);
    }
    None
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
    let inputs = [
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_CONTROL,
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
                    wVk: VK_CONTROL,
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
