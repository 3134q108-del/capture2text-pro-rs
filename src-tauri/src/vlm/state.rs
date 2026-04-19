use serde::Serialize;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize)]
pub struct VlmSnapshot {
    pub source: String,
    pub status: String,
    pub original: String,
    pub translated: String,
    pub duration_ms: u64,
    pub error: Option<String>,
    pub updated_at: u64,
}

static LATEST: OnceLock<Mutex<Option<VlmSnapshot>>> = OnceLock::new();

pub fn init() {
    let _ = LATEST.get_or_init(|| Mutex::new(None));
}

pub fn set_loading(source: impl AsRef<str>) {
    update(VlmSnapshot {
        source: source.as_ref().to_string(),
        status: "loading".to_string(),
        original: String::new(),
        translated: String::new(),
        duration_ms: 0,
        error: None,
        updated_at: now_unix_millis(),
    });
}

pub fn set_partial(source: impl AsRef<str>, original: impl AsRef<str>, translated: impl AsRef<str>) {
    eprintln!("[state] set_partial source={}", source.as_ref());
    update(VlmSnapshot {
        source: source.as_ref().to_string(),
        status: "loading".to_string(),
        original: original.as_ref().to_string(),
        translated: translated.as_ref().to_string(),
        duration_ms: 0,
        error: None,
        updated_at: now_unix_millis(),
    });
}

pub fn set_success(
    source: impl AsRef<str>,
    original: impl AsRef<str>,
    translated: impl AsRef<str>,
    duration_ms: u64,
) {
    eprintln!(
        "[state] set_success source={} original.len={}",
        source.as_ref(),
        original.as_ref().len()
    );
    update(VlmSnapshot {
        source: source.as_ref().to_string(),
        status: "success".to_string(),
        original: original.as_ref().to_string(),
        translated: translated.as_ref().to_string(),
        duration_ms,
        error: None,
        updated_at: now_unix_millis(),
    });
}

pub fn set_error(source: impl AsRef<str>, error: impl Into<String>) {
    eprintln!("[state] set_error source={}", source.as_ref());
    update(VlmSnapshot {
        source: source.as_ref().to_string(),
        status: "error".to_string(),
        original: String::new(),
        translated: String::new(),
        duration_ms: 0,
        error: Some(error.into()),
        updated_at: now_unix_millis(),
    });
}

pub fn snapshot() -> Option<VlmSnapshot> {
    let slot = LATEST.get_or_init(|| Mutex::new(None));
    let guard = slot.lock();
    let is_some = guard.as_ref().ok().map(|g| g.is_some()).unwrap_or(false);
    eprintln!("[state] snapshot get: is_some={}", is_some);
    match guard {
        Ok(guard) => guard.clone(),
        Err(_) => None,
    }
}

fn update(next: VlmSnapshot) {
    let slot = LATEST.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = slot.lock() {
        *guard = Some(next);
    }
}

fn now_unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
