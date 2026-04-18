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
use std::time::Instant;

const HOTKEY_CHANNEL_CAPACITY: usize = 8;

static CAPTURE_TX: OnceLock<SyncSender<CaptureRequest>> = OnceLock::new();

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
pub struct ScreenRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Clone, Copy, Debug)]
pub enum CaptureRequest {
    Hotkey {
        kind: HotkeyKind,
        cursor: CursorPoint,
        queued_at: Instant,
    },
    SelectedRect {
        rect: ScreenRect,
        queued_at: Instant,
    },
}

pub fn start_worker() -> io::Result<()> {
    let (tx, rx) = sync_channel(HOTKEY_CHANNEL_CAPACITY);

    CAPTURE_TX
        .set(tx)
        .map_err(|_| io::Error::new(io::ErrorKind::AlreadyExists, "hotkey sender already initialized"))?;

    thread::Builder::new()
        .name("capture-worker".to_string())
        .spawn(move || screenshot::worker_loop(rx))?;

    Ok(())
}

pub(crate) fn try_enqueue_from_hook(kind: HotkeyKind, cursor: CursorPoint, queued_at: Instant) {
    try_enqueue_request(CaptureRequest::Hotkey {
        kind,
        cursor,
        queued_at,
    });
}

pub(crate) fn try_enqueue_request(request: CaptureRequest) {
    if let Some(tx) = CAPTURE_TX.get() {
        let _ = tx.try_send(request);
    }
}
