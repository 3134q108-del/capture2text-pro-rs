use std::sync::OnceLock;
use tauri::AppHandle;

static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

pub fn set(handle: AppHandle) {
    let _ = APP_HANDLE.set(handle);
}

pub fn get() -> Option<AppHandle> {
    APP_HANDLE.get().cloned()
}
