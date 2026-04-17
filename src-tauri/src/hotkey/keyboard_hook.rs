use std::io;
use std::sync::mpsc;
use std::thread;

use crate::capture::{self, HotkeyKind};

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    KEYEVENTF_KEYUP, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT,
    MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
};

const VK_Q: u32 = 0x51;
const VK_W: u32 = 0x57;
const VK_E: u32 = 0x45;

pub fn install() -> io::Result<()> {
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);

    thread::Builder::new()
        .name("keyboard-hook".to_string())
        .spawn(move || {
            let hook_result = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), None, 0) };

            match hook_result {
                Ok(hook) => {
                    println!(
                        "[hotkey] WH_KEYBOARD_LL installed, thread id={}",
                        unsafe { GetCurrentThreadId() }
                    );
                    let _ = ready_tx.send(Ok(()));
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

    ready_rx.recv().unwrap_or_else(|_| {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "keyboard hook thread exited before initialization",
        ))
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
    if message == WM_KEYDOWN || message == WM_SYSKEYDOWN {
        let kbd = unsafe { *(lparam.0 as *const KBDLLHOOKSTRUCT) };
        let vk = kbd.vkCode;

        let ctrl_down = key_down(i32::from(VK_CONTROL.0));
        let shift_down = key_down(i32::from(VK_SHIFT.0));
        let win_down = key_down(i32::from(VK_LWIN.0)) || key_down(i32::from(VK_RWIN.0));
        let alt_down = key_down(i32::from(VK_MENU.0));

        let is_target = matches!(vk, VK_Q | VK_W | VK_E);
        if is_target && win_down && !ctrl_down && !shift_down && !alt_down {
            let kind = match vk {
                VK_Q => HotkeyKind::Q,
                VK_W => HotkeyKind::W,
                VK_E => HotkeyKind::E,
                _ => unreachable!(),
            };
            capture::try_enqueue_from_hook(kind);

            if !ctrl_down && !shift_down && ((win_down && !alt_down) || (alt_down && !win_down)) {
                unsafe { send_ctrl_tap() };
            }

            return LRESULT(1);
        }
    }

    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn key_down(vk: i32) -> bool {
    (unsafe { GetKeyState(vk) } as u16 & 0x8000) != 0
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
