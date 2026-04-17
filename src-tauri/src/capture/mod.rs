pub mod screenshot;
#[allow(dead_code)]
pub mod preprocess;

use std::io;
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::OnceLock;
use std::thread;

const HOTKEY_CHANNEL_CAPACITY: usize = 8;

static HOTKEY_TX: OnceLock<SyncSender<HotkeyKind>> = OnceLock::new();

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

pub fn start_worker() -> io::Result<()> {
    let (tx, rx) = sync_channel(HOTKEY_CHANNEL_CAPACITY);

    HOTKEY_TX
        .set(tx)
        .map_err(|_| io::Error::new(io::ErrorKind::AlreadyExists, "hotkey sender already initialized"))?;

    thread::Builder::new()
        .name("capture-worker".to_string())
        .spawn(move || screenshot::worker_loop(rx))?;

    Ok(())
}

pub(crate) fn try_enqueue_from_hook(kind: HotkeyKind) {
    if let Some(tx) = HOTKEY_TX.get() {
        let _ = tx.try_send(kind);
    }
}
