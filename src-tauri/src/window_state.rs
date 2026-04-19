use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowState {
    pub popup_width: u32,
    pub popup_height: u32,
    pub popup_x: Option<i32>,
    pub popup_y: Option<i32>,
    pub popup_topmost: bool,
    pub popup_font: Option<String>,
    pub popup_show_enabled: bool,
    pub save_to_clipboard: bool,
    pub translate_append_to_clipboard: bool,
    pub translate_separator: String,
    pub capture_box_border_rgba: [u8; 4],
    pub capture_box_fill_rgba: [u8; 4],
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            popup_width: 661,
            popup_height: 371,
            popup_x: None,
            popup_y: None,
            popup_topmost: true,
            popup_font: None,
            popup_show_enabled: true,
            save_to_clipboard: true,
            translate_append_to_clipboard: false,
            translate_separator: "Space".to_string(),
            capture_box_border_rgba: [255, 0, 0, 255],
            capture_box_fill_rgba: [255, 0, 0, 64],
        }
    }
}

static WINDOW_STATE: OnceLock<Mutex<WindowState>> = OnceLock::new();

pub fn init_runtime() {
    let _ = WINDOW_STATE.get_or_init(|| Mutex::new(load_or_default()));
}

pub fn get() -> WindowState {
    let slot = WINDOW_STATE.get_or_init(|| Mutex::new(load_or_default()));
    match slot.lock() {
        Ok(guard) => guard.clone(),
        Err(_) => WindowState::default(),
    }
}

pub fn set_popup_size(w: u32, h: u32) {
    update(|state| {
        state.popup_width = w;
        state.popup_height = h;
    });
}

pub fn set_popup_position(x: i32, y: i32) {
    update(|state| {
        state.popup_x = Some(x);
        state.popup_y = Some(y);
    });
}

pub fn set_popup_topmost(v: bool) {
    update(|state| {
        state.popup_topmost = v;
    });
}

fn update(mutator: impl FnOnce(&mut WindowState)) {
    let slot = WINDOW_STATE.get_or_init(|| Mutex::new(load_or_default()));
    if let Ok(mut guard) = slot.lock() {
        mutator(&mut guard);
        persist_best_effort(&guard);
    }
}

fn load_or_default() -> WindowState {
    let state = load_from_disk().unwrap_or_default();
    persist_best_effort(&state);
    state
}

fn load_from_disk() -> Option<WindowState> {
    let path = storage_path().ok()?;
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str::<WindowState>(&raw).ok()
}

fn persist_best_effort(state: &WindowState) {
    let Ok(path) = storage_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(raw) = serde_json::to_string_pretty(state) {
        let _ = fs::write(path, raw);
    }
}

fn storage_path() -> std::io::Result<PathBuf> {
    let local = dirs::data_local_dir().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "local appdata not found")
    })?;
    Ok(local.join("Capture2TextPro").join("window_state.json"))
}
