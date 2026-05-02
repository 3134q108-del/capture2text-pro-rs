use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PopupFont {
    pub family: String,
    pub size_pt: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct HotkeyModifiers {
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub win: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct HotkeyBinding {
    pub modifiers: HotkeyModifiers,
    pub key_code: u32,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BillingTier {
    F0,
    S0,
}

impl Default for BillingTier {
    fn default() -> Self {
        Self::F0
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum TranslationMode {
    Smart,
    Direct,
}

impl Default for TranslationMode {
    fn default() -> Self {
        Self::Smart
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
    #[serde(default)]
    pub azure_voice_map: HashMap<String, String>,
    #[serde(default = "default_speech_rate")]
    pub azure_speech_rate: f32,
    #[serde(default = "default_speech_volume")]
    pub azure_speech_volume: f32,
    #[serde(default)]
    pub azure_billing_tier: BillingTier,
    #[serde(default)]
    pub azure_usage_neural_chars: u64,
    #[serde(default)]
    pub azure_usage_hd_chars: u64,
    #[serde(default)]
    pub azure_usage_month: String,
    #[serde(default = "default_neural_limit")]
    pub azure_neural_limit: u64,
    #[serde(default = "default_hd_limit")]
    pub azure_hd_limit: u64,
    #[serde(default = "default_hotkey_q")]
    pub hotkey_q: HotkeyBinding,
    #[serde(default = "default_hotkey_w")]
    pub hotkey_w: HotkeyBinding,
    #[serde(default = "default_hotkey_e")]
    pub hotkey_e: HotkeyBinding,
    #[serde(default = "default_native_lang")]
    pub native_lang: String,
    #[serde(default = "default_target_lang")]
    pub target_lang: String,
    #[serde(default = "default_enabled_langs")]
    pub enabled_langs: Vec<String>,
    #[serde(default)]
    pub translation_mode: TranslationMode,
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
            azure_voice_map: HashMap::new(),
            azure_speech_rate: default_speech_rate(),
            azure_speech_volume: default_speech_volume(),
            azure_billing_tier: BillingTier::default(),
            azure_usage_neural_chars: 0,
            azure_usage_hd_chars: 0,
            azure_usage_month: String::new(),
            azure_neural_limit: default_neural_limit(),
            azure_hd_limit: default_hd_limit(),
            hotkey_q: default_hotkey_q(),
            hotkey_w: default_hotkey_w(),
            hotkey_e: default_hotkey_e(),
            native_lang: default_native_lang(),
            target_lang: default_target_lang(),
            enabled_langs: default_enabled_langs(),
            translation_mode: TranslationMode::default(),
        }
    }
}

fn default_speech_enabled() -> bool {
    true
}

fn default_active_preset() -> String {
    "Ryan".to_string()
}

fn default_speech_rate() -> f32 {
    1.0
}

fn default_speech_volume() -> f32 {
    1.0
}

fn default_neural_limit() -> u64 {
    1_000_000
}

fn default_hd_limit() -> u64 {
    100_000
}

fn default_hotkey_q() -> HotkeyBinding {
    HotkeyBinding {
        modifiers: HotkeyModifiers {
            win: true,
            ..HotkeyModifiers::default()
        },
        key_code: 0x51,
    }
}

fn default_hotkey_w() -> HotkeyBinding {
    HotkeyBinding {
        modifiers: HotkeyModifiers {
            win: true,
            ..HotkeyModifiers::default()
        },
        key_code: 0x57,
    }
}

fn default_hotkey_e() -> HotkeyBinding {
    HotkeyBinding {
        modifiers: HotkeyModifiers {
            win: true,
            ..HotkeyModifiers::default()
        },
        key_code: 0x45,
    }
}

fn default_log_enabled() -> bool {
    false
}

fn default_clipboard_mode() -> ClipboardMode {
    ClipboardMode::OriginalOnly
}

fn default_log_file_path() -> String {
    crate::app_paths::data_dir()
        .join("captures.log")
        .to_string_lossy()
        .to_string()
}

fn default_native_lang() -> String {
    "zh-TW".to_string()
}

fn default_target_lang() -> String {
    "en-US".to_string()
}

fn default_enabled_langs() -> Vec<String> {
    vec![
        "zh-CN".to_string(),
        "zh-TW".to_string(),
        "en-US".to_string(),
        "ja-JP".to_string(),
        "ko-KR".to_string(),
    ]
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

pub fn azure_voice_map() -> HashMap<String, String> {
    get().azure_voice_map
}

pub fn set_azure_voice_for_lang(lang: String, voice_id: String) {
    update(|state| {
        state.azure_voice_map.insert(lang, voice_id);
    });
}

pub fn clear_azure_voice_for_lang(lang: &str) {
    update(|state| {
        state.azure_voice_map.remove(lang);
    });
}

pub fn azure_speech_rate() -> f32 {
    get().azure_speech_rate.clamp(0.5, 2.0)
}

pub fn set_azure_speech_rate(rate: f32) {
    update(|state| {
        state.azure_speech_rate = rate.clamp(0.5, 2.0);
    });
}

pub fn azure_speech_volume() -> f32 {
    get().azure_speech_volume.clamp(0.5, 2.0)
}

pub fn set_azure_speech_volume(volume: f32) {
    update(|state| {
        state.azure_speech_volume = volume.clamp(0.5, 2.0);
    });
}

pub fn record_usage(voice_id: &str, chars: u64) {
    update(|state| {
        record_usage_inner(
            state,
            voice_id,
            chars,
            crate::azure_tts::usage::current_month(),
        );
    });
}

fn record_usage_inner(state: &mut WindowState, voice_id: &str, chars: u64, now_month: String) {
    if state.azure_usage_month != now_month {
        state.azure_usage_neural_chars = 0;
        state.azure_usage_hd_chars = 0;
        state.azure_usage_month = now_month;
    }
    if crate::azure_tts::usage::is_hd_voice(voice_id) {
        state.azure_usage_hd_chars = state.azure_usage_hd_chars.saturating_add(chars);
    } else {
        state.azure_usage_neural_chars = state.azure_usage_neural_chars.saturating_add(chars);
    }
}

pub fn azure_billing_tier() -> BillingTier {
    get().azure_billing_tier
}

pub fn set_azure_billing_tier(tier: BillingTier) {
    update(|state| {
        state.azure_billing_tier = tier;
    });
}

pub fn azure_usage_snapshot() -> (u64, u64, String) {
    let state = get();
    let now_month = crate::azure_tts::usage::current_month();
    if state.azure_usage_month != now_month {
        (0, 0, now_month)
    } else {
        (
            state.azure_usage_neural_chars,
            state.azure_usage_hd_chars,
            state.azure_usage_month,
        )
    }
}

pub fn azure_neural_limit() -> u64 {
    get().azure_neural_limit.max(1)
}

pub fn set_azure_neural_limit(limit: u64) {
    update(|state| {
        state.azure_neural_limit = limit.max(1);
    });
}

pub fn azure_hd_limit() -> u64 {
    get().azure_hd_limit.max(1)
}

pub fn set_azure_hd_limit(limit: u64) {
    update(|state| {
        state.azure_hd_limit = limit.max(1);
    });
}

pub fn set_clipboard_mode(v: ClipboardMode) {
    update(|state| {
        state.clipboard_mode = v;
        sync_legacy_clipboard_fields(state);
    });
}

pub fn hotkey_q() -> HotkeyBinding {
    get().hotkey_q
}

pub fn hotkey_w() -> HotkeyBinding {
    get().hotkey_w
}

pub fn hotkey_e() -> HotkeyBinding {
    get().hotkey_e
}

pub fn set_hotkey_q(binding: HotkeyBinding) {
    update(|state| {
        state.hotkey_q = binding;
    });
}

pub fn set_hotkey_w(binding: HotkeyBinding) {
    update(|state| {
        state.hotkey_w = binding;
    });
}

pub fn set_hotkey_e(binding: HotkeyBinding) {
    update(|state| {
        state.hotkey_e = binding;
    });
}

pub fn native_lang() -> String {
    get().native_lang
}

pub fn target_lang() -> String {
    get().target_lang
}

pub fn enabled_langs() -> Vec<String> {
    get().enabled_langs
}

pub fn translation_mode() -> TranslationMode {
    get().translation_mode
}

pub fn set_translation_mode(mode: TranslationMode) {
    update(|state| {
        state.translation_mode = mode;
    });
}

pub fn set_native_lang(lang: String) -> Result<(), String> {
    let state = get();
    set_language_preferences(lang, state.target_lang, state.enabled_langs)
}

pub fn set_target_lang(lang: String) -> Result<(), String> {
    let state = get();
    set_language_preferences(state.native_lang, lang, state.enabled_langs)
}

pub fn set_enabled_langs(langs: Vec<String>) -> Result<(), String> {
    let state = get();
    set_language_preferences(state.native_lang, state.target_lang, langs)
}

pub fn set_language_preferences(
    native_lang: String,
    target_lang: String,
    enabled_langs: Vec<String>,
) -> Result<(), String> {
    update_result(|state| apply_language_preferences(state, native_lang, target_lang, enabled_langs))
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

fn update_result(mutator: impl FnOnce(&mut WindowState) -> Result<(), String>) -> Result<(), String> {
    let slot = WINDOW_STATE.get_or_init(|| Mutex::new(load_or_default()));
    let snapshot = if let Ok(mut guard) = slot.lock() {
        mutator(&mut guard)?;
        persist_best_effort(&guard);
        Some(guard.clone())
    } else {
        return Err("window_state lock poisoned".to_string());
    };

    if let Some(snap) = snapshot {
        if let Some(app) = crate::app_handle::get() {
            use tauri::Emitter;
            let _ = app.emit("window-state-changed", &snap);
        }
    }
    Ok(())
}

fn load_or_default() -> WindowState {
    let mut state = load_from_disk().unwrap_or_default();
    migrate_legacy_output_lang(&mut state);
    sanitize_clipboard_mode(&mut state);
    if state.speech_active_preset.trim().is_empty() {
        state.speech_active_preset = default_active_preset();
    }
    let native = state.native_lang.clone();
    let target = state.target_lang.clone();
    let enabled = state.enabled_langs.clone();
    if apply_language_preferences(&mut state, native, target, enabled).is_err() {
        let defaults = default_enabled_langs();
        state.native_lang = default_native_lang();
        state.target_lang = default_target_lang();
        state.enabled_langs = defaults;
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
    Ok(crate::app_paths::data_dir().join("window_state.json"))
}

fn legacy_output_lang_path() -> std::io::Result<PathBuf> {
    Ok(crate::app_paths::data_dir().join("output_lang.txt"))
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

fn normalize_language_code(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let canonical_guess = match trimmed.to_ascii_lowercase().as_str() {
        "zh" | "zh-tw" => "zh-TW",
        "zh-cn" => "zh-CN",
        "en" | "en-us" => "en-US",
        "ja" | "ja-jp" => "ja-JP",
        "ko" | "ko-kr" => "ko-KR",
        "fr" | "fr-fr" => "fr-FR",
        "de" | "de-de" => "de-DE",
        _ => trimmed,
    };

    if let Some(lang) = crate::languages::by_code(canonical_guess) {
        return Some(lang.code.as_str().to_string());
    }

    crate::languages::all()
        .iter()
        .find(|lang| lang.code.as_str().eq_ignore_ascii_case(canonical_guess))
        .map(|lang| lang.code.as_str().to_string())
}

fn dedup_language_codes(codes: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for code in codes {
        if !out.iter().any(|existing| existing == &code) {
            out.push(code);
        }
    }
    out
}

fn apply_language_preferences(
    state: &mut WindowState,
    native_lang: String,
    target_lang: String,
    enabled_langs: Vec<String>,
) -> Result<(), String> {
    let native = normalize_language_code(&native_lang)
        .ok_or_else(|| format!("invalid native_lang: {}", native_lang))?;
    let target = normalize_language_code(&target_lang)
        .ok_or_else(|| format!("invalid target_lang: {}", target_lang))?;

    let mut enabled = Vec::new();
    for code in enabled_langs {
        let normalized = normalize_language_code(&code)
            .ok_or_else(|| format!("invalid enabled_lang code: {}", code))?;
        enabled.push(normalized);
    }
    let enabled = dedup_language_codes(enabled);
    if enabled.is_empty() {
        return Err("enabled_langs must not be empty".to_string());
    }
    if !enabled.iter().any(|code| code == &native) {
        return Err("native_lang must be included in enabled_langs".to_string());
    }
    if !enabled.iter().any(|code| code == &target) {
        return Err("target_lang must be included in enabled_langs".to_string());
    }

    state.native_lang = native;
    state.target_lang = target;
    state.enabled_langs = enabled;
    Ok(())
}

fn migrate_legacy_output_lang(state: &mut WindowState) {
    if state.enabled_langs.is_empty() {
        state.enabled_langs = default_enabled_langs();
    }

    let Ok(path) = legacy_output_lang_path() else {
        return;
    };
    if !path.exists() {
        return;
    }
    let Ok(raw) = fs::read_to_string(path) else {
        return;
    };
    let Some(legacy) = normalize_language_code(raw.trim()) else {
        return;
    };

    let mut next = state.enabled_langs.clone();
    next.push(legacy.clone());
    state.enabled_langs = dedup_language_codes(next);

    if state.target_lang.trim().is_empty() {
        state.target_lang = legacy;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_usage_inner_resets_month_and_splits_hd_counter() {
        let mut state = WindowState {
            azure_usage_month: "2026-03".to_string(),
            azure_usage_neural_chars: 100,
            azure_usage_hd_chars: 200,
            ..WindowState::default()
        };

        record_usage_inner(
            &mut state,
            "en-US-Ava:DragonHDLatestNeural",
            42,
            "2026-04".to_string(),
        );

        assert_eq!(state.azure_usage_month, "2026-04");
        assert_eq!(state.azure_usage_neural_chars, 0);
        assert_eq!(state.azure_usage_hd_chars, 42);

        record_usage_inner(
            &mut state,
            "en-US-AvaMultilingualNeural",
            7,
            "2026-04".to_string(),
        );
        assert_eq!(state.azure_usage_neural_chars, 7);
        assert_eq!(state.azure_usage_hd_chars, 42);
    }

    #[test]
    fn language_preferences_require_non_empty_enabled_langs() {
        let mut state = WindowState::default();
        let result = apply_language_preferences(
            &mut state,
            "zh-TW".to_string(),
            "en-US".to_string(),
            vec![],
        );
        assert!(result.is_err());
    }

    #[test]
    fn language_preferences_require_native_and_target_in_enabled() {
        let mut state = WindowState::default();
        let result = apply_language_preferences(
            &mut state,
            "zh-TW".to_string(),
            "en-US".to_string(),
            vec!["zh-TW".to_string()],
        );
        assert!(result.is_err());
    }

    #[test]
    fn language_preferences_reject_unknown_language_code() {
        let mut state = WindowState::default();
        let result = apply_language_preferences(
            &mut state,
            "xx-XX".to_string(),
            "en-US".to_string(),
            vec!["zh-TW".to_string(), "en-US".to_string()],
        );
        assert!(result.is_err());
    }
}
