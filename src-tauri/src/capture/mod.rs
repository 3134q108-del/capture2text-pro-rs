pub mod screenshot;
#[allow(dead_code)]
pub mod preprocess;
pub mod log;
pub mod params;
pub mod pipeline;
pub mod screen_capture;

use std::io;
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::{Mutex, OnceLock};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Instant;
use std::time::Duration;

const HOTKEY_CHANNEL_CAPACITY: usize = 8;
const CAPTURE_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

static CAPTURE_RUNTIME: OnceLock<CaptureRuntime> = OnceLock::new();
static LATEST_REQUEST_SEQ: AtomicU64 = AtomicU64::new(0);

struct CaptureRuntime {
    tx: SyncSender<CaptureRequest>,
    join: Mutex<Option<JoinHandle<()>>>,
}

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
        seq: u64,
        kind: HotkeyKind,
        cursor: CursorPoint,
        queued_at: Instant,
    },
    SelectedRect {
        seq: u64,
        rect: ScreenRect,
        queued_at: Instant,
    },
    Exit,
}

pub fn start_worker() -> io::Result<()> {
    let (tx, rx) = sync_channel(HOTKEY_CHANNEL_CAPACITY);

    let join = thread::Builder::new()
        .name("capture-worker".to_string())
        .spawn(move || screenshot::worker_loop(rx))?;

    CAPTURE_RUNTIME
        .set(CaptureRuntime {
            tx,
            join: Mutex::new(Some(join)),
        })
        .map_err(|_| io::Error::new(io::ErrorKind::AlreadyExists, "capture runtime already initialized"))?;

    Ok(())
}

pub(crate) fn try_enqueue_from_hook(kind: HotkeyKind, cursor: CursorPoint, queued_at: Instant) {
    try_enqueue_request(CaptureRequest::Hotkey {
        seq: next_request_seq(),
        kind,
        cursor,
        queued_at,
    });
}

pub(crate) fn try_enqueue_request(request: CaptureRequest) {
    if let Some(runtime) = CAPTURE_RUNTIME.get() {
        let _ = runtime.tx.try_send(request);
    }
}

pub(crate) fn next_request_seq() -> u64 {
    LATEST_REQUEST_SEQ.fetch_add(1, Ordering::SeqCst) + 1
}

pub(crate) fn latest_request_seq() -> u64 {
    LATEST_REQUEST_SEQ.load(Ordering::SeqCst)
}

pub fn shutdown_worker() {
    let Some(runtime) = CAPTURE_RUNTIME.get() else {
        return;
    };

    let _ = runtime.tx.send(CaptureRequest::Exit);

    let join = runtime.join.lock().ok().and_then(|mut guard| guard.take());
    if let Some(join) = join {
        let (done_tx, done_rx) = sync_channel(1);
        let _ = thread::Builder::new()
            .name("capture-join-wait".to_string())
            .spawn(move || {
                let _ = join.join();
                let _ = done_tx.send(());
            });
        let _ = done_rx.recv_timeout(CAPTURE_SHUTDOWN_TIMEOUT);
    }
}
