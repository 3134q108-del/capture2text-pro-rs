pub mod screenshot;
#[allow(dead_code)]
pub mod preprocess;
pub mod params;
pub mod pipeline;
pub mod screen_capture;

use std::io;
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::OnceLock;
use std::thread;

use tauri::AppHandle;

const HOTKEY_CHANNEL_CAPACITY: usize = 8;

static HOTKEY_TX: OnceLock<SyncSender<HotkeyEvent>> = OnceLock::new();

#[derive(Clone, Copy, Debug)]
pub enum HotkeyKind {
    Q,
    W,
    E,
}

impl HotkeyKind {
    pub fn as_suffix(self) -> &'static str {
        match self {
            Self::Q => "q",
            Self::W => "w",
            Self::E => "e",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CursorPoint {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug)]
pub struct HotkeyEvent {
    pub kind: HotkeyKind,
    pub cursor: CursorPoint,
}

pub fn start_worker(app: AppHandle) -> io::Result<()> {
    let (tx, rx) = sync_channel(HOTKEY_CHANNEL_CAPACITY);

    HOTKEY_TX
        .set(tx)
        .map_err(|_| io::Error::new(io::ErrorKind::AlreadyExists, "hotkey sender already initialized"))?;

    thread::Builder::new()
        .name("capture-worker".to_string())
        .spawn(move || screenshot::worker_loop(app, rx))?;

    Ok(())
}

pub(crate) fn try_enqueue_from_hook(event: HotkeyEvent) {
    if let Some(tx) = HOTKEY_TX.get() {
        let _ = tx.try_send(event);
    }
}
