use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PopupFont {
    pub family: String,
    pub size_pt: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum ClipboardMode {
    None,
    OriginalOnly,
    TranslatedOnly,
    Both,
}

impl ClipboardMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ClipboardMode::None => "None",
            ClipboardMode::OriginalOnly => "OriginalOnly",
            ClipboardMode::TranslatedOnly => "TranslatedOnly",
            ClipboardMode::Both => "Both",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "None" => Some(Self::None),
            "OriginalOnly" => Some(Self::OriginalOnly),
            "TranslatedOnly" => Some(Self::TranslatedOnly),
            "Both" => Some(Self::Both),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowState {
    pub popup_width: u32,
    pub popup_height: u32,
    pub popup_x: Option<i32>,
    pub popup_y: Option<i32>,
    pub popup_topmost: bool,
    pub popup_font: Option<PopupFont>,
    pub popup_show_enabled: bool,
    #[serde(default = "default_clipboard_mode")]
    pub clipboard_mode: ClipboardMode,
    pub save_to_clipboard: bool,
    pub translate_append_to_clipboard: bool,
    pub translate_separator: String,
    #[serde(default = "default_log_enabled")]
    pub log_enabled: bool,
    #[serde(default = "default_log_file_path")]
    pub log_file_path: String,
    pub capture_box_border_rgba: [u8; 4],
    pub capture_box_fill_rgba: [u8; 4],
    #[serde(default = "default_speech_enabled")]
    pub speech_enabled: bool,
    #[serde(default = "default_active_preset")]
    pub speech_active_preset: String,
    #[serde(default)]
    pub azure_region: Option<String>,
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
            clipboard_mode: default_clipboard_mode(),
            save_to_clipboard: true,
            translate_append_to_clipboard: false,
            translate_separator: "Space".to_string(),
            log_enabled: default_log_enabled(),
            log_file_path: default_log_file_path(),
            capture_box_border_rgba: [255, 0, 0, 255],
            capture_box_fill_rgba: [255, 0, 0, 64],
            speech_enabled: default_speech_enabled(),
            speech_active_preset: default_active_preset(),
            azure_region: None,
        }
    }
}

fn default_speech_enabled() -> bool {
    true
}

fn default_active_preset() -> String {
    "Ryan".to_string()
}

fn default_log_enabled() -> bool {
    false
}

fn default_clipboard_mode() -> ClipboardMode {
    ClipboardMode::OriginalOnly
}

fn default_log_file_path() -> String {
    let base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("Capture2TextPro")
        .join("captures.log")
        .to_string_lossy()
        .to_string()
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

pub fn set_popup_font(font: Option<PopupFont>) {
    update(|state| {
        state.popup_font = font;
    });
}

pub fn set_save_to_clipboard(v: bool) {
    update(|state| {
        state.save_to_clipboard = v;
        state.clipboard_mode =
            clipboard_mode_from_legacy(state.save_to_clipboard, state.translate_append_to_clipboard);
    });
}

pub fn set_popup_show_enabled(v: bool) {
    update(|state| {
        state.popup_show_enabled = v;
    });
}

pub fn set_translate_append_to_clipboard(v: bool) {
    update(|state| {
        state.translate_append_to_clipboard = v;
        state.clipboard_mode =
            clipboard_mode_from_legacy(state.save_to_clipboard, state.translate_append_to_clipboard);
    });
}

pub fn set_translate_separator(v: String) {
    update(|state| {
        state.translate_separator = v;
    });
}

pub fn set_log_enabled(v: bool) {
    update(|state| {
        state.log_enabled = v;
    });
}

pub fn set_log_file_path(v: String) {
    update(|state| {
        state.log_file_path = v;
    });
}

pub fn set_speech_enabled(v: bool) {
    update(|state| {
        state.speech_enabled = v;
    });
}

pub fn set_speech_active_preset(v: String) {
    update(|state| {
        state.speech_active_preset = v;
    });
}

pub fn azure_region() -> Option<String> {
    get().azure_region
}

pub fn set_azure_region(v: Option<String>) {
    update(|state| {
        state.azure_region = v;
    });
}

pub fn set_clipboard_mode(v: ClipboardMode) {
    update(|state| {
        state.clipboard_mode = v;
        sync_legacy_clipboard_fields(state);
    });
}

fn update(mutator: impl FnOnce(&mut WindowState)) {
    let slot = WINDOW_STATE.get_or_init(|| Mutex::new(load_or_default()));
    let snapshot = if let Ok(mut guard) = slot.lock() {
        mutator(&mut guard);
        persist_best_effort(&guard);
        Some(guard.clone())
    } else {
        None
    };

    if let Some(snap) = snapshot {
        if let Some(app) = crate::app_handle::get() {
            use tauri::Emitter;
            let _ = app.emit("window-state-changed", &snap);
        }
    }
}

fn load_or_default() -> WindowState {
    let mut state = load_from_disk().unwrap_or_default();
    sanitize_clipboard_mode(&mut state);
    if state.speech_active_preset.trim().is_empty() {
        state.speech_active_preset = default_active_preset();
    }
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
        let tmp_path = path.with_extension(format!("json.tmp.{}", std::process::id()));
        if let Ok(mut file) = fs::File::create(&tmp_path) {
            if file.write_all(raw.as_bytes()).is_ok() && file.sync_all().is_ok() {
                drop(file);
                let _ = atomic_replace(&tmp_path, &path);
            } else {
                let _ = fs::remove_file(&tmp_path);
            }
        }
    }
}

#[cfg(windows)]
fn atomic_replace(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let from_wide: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to_wide: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        MoveFileExW(
            PCWSTR(from_wide.as_ptr()),
            PCWSTR(to_wide.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err.to_string()))
    }
}

#[cfg(not(windows))]
fn atomic_replace(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    fs::rename(from, to)
}

fn storage_path() -> std::io::Result<PathBuf> {
    let local = dirs::data_local_dir().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "local appdata not found")
    })?;
    Ok(local.join("Capture2TextPro").join("window_state.json"))
}

fn clipboard_mode_from_legacy(save_to_clipboard: bool, translate_append_to_clipboard: bool) -> ClipboardMode {
    if !save_to_clipboard {
        ClipboardMode::None
    } else if translate_append_to_clipboard {
        ClipboardMode::Both
    } else {
        ClipboardMode::OriginalOnly
    }
}

fn sync_legacy_clipboard_fields(state: &mut WindowState) {
    match state.clipboard_mode {
        ClipboardMode::None => {
            state.save_to_clipboard = false;
            state.translate_append_to_clipboard = false;
        }
        ClipboardMode::OriginalOnly => {
            state.save_to_clipboard = true;
            state.translate_append_to_clipboard = false;
        }
        ClipboardMode::TranslatedOnly => {
            state.save_to_clipboard = true;
            state.translate_append_to_clipboard = false;
        }
        ClipboardMode::Both => {
            state.save_to_clipboard = true;
            state.translate_append_to_clipboard = true;
        }
    }
}

fn sanitize_clipboard_mode(state: &mut WindowState) {
    if state.clipboard_mode == default_clipboard_mode()
        && (!state.save_to_clipboard || state.translate_append_to_clipboard)
    {
        state.clipboard_mode =
            clipboard_mode_from_legacy(state.save_to_clipboard, state.translate_append_to_clipboard);
    }
    sync_legacy_clipboard_fields(state);
}
