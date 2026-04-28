use std::sync::{Arc, Mutex};

use tauri::AppHandle;
use tokio::task::JoinHandle;

use super::playback::PlaybackController;

pub struct TtsRuntime {
    pub app: AppHandle,
    pub playback: PlaybackController,
    pub current_task: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl TtsRuntime {
    pub fn new(app: &AppHandle) -> Self {
        Self {
            app: app.clone(),
            playback: PlaybackController::new(app.clone()),
            current_task: Arc::new(Mutex::new(None)),
        }
    }
}
