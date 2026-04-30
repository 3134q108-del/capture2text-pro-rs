use std::fs;
use std::path::PathBuf;

pub const APP_NAME: &str = "com.capture2text.pro";
pub const LEGACY_NAME: &str = "Capture2TextPro";

pub fn data_dir() -> PathBuf {
    let local = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    let new = local.join(APP_NAME);
    if new.exists() {
        return new;
    }
    let old = local.join(LEGACY_NAME);
    if old.exists() {
        return old;
    }
    new
}

pub fn ensure_migration() {
    let Some(local) = dirs::data_local_dir() else {
        eprintln!("[migration] no data_local_dir, skip");
        return;
    };
    let old = local.join(LEGACY_NAME);
    let new = local.join(APP_NAME);
    if !old.exists() || new.exists() {
        return;
    }
    match fs::rename(&old, &new) {
        Ok(_) => eprintln!("[migration] moved {} -> {}", old.display(), new.display()),
        Err(e) => eprintln!(
            "[migration] rename failed (data_dir() will fallback to legacy): {e}"
        ),
    }
}
