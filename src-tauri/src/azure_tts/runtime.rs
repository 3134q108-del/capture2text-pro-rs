use std::sync::{Arc, Mutex};

use tokio::task::JoinHandle;

use super::playback::PlaybackController;

pub struct TtsRuntime {
    pub playback: PlaybackController,
    pub current_task: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl TtsRuntime {
    pub fn new() -> Self {
        Self {
            playback: PlaybackController::new(),
            current_task: Arc::new(Mutex::new(None)),
        }
    }
}
