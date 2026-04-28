use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Emitter};

static INSTALLING_PIXTRAL: AtomicBool = AtomicBool::new(false);

#[tauri::command]
pub fn check_pixtral_installed() -> bool {
    crate::llama_runtime::is_pixtral_installed()
}

#[tauri::command]
pub fn list_available_output_langs() -> Vec<String> {
    crate::output_lang::available_langs()
}

#[tauri::command]
pub async fn install_pixtral(app: AppHandle) -> Result<(), String> {
    if crate::llama_runtime::is_pixtral_installed() {
        return Err("already installed".to_string());
    }

    if INSTALLING_PIXTRAL
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("install already in progress".to_string());
    }

    tauri::async_runtime::spawn(async move {
        let result = tauri::async_runtime::spawn_blocking({
            let app = app.clone();
            move || crate::llama_runtime::install_pixtral_with_progress(&app)
        })
        .await
        .map_err(|err| err.to_string())
        .and_then(|result| result);

        match result {
            Ok(_) => {
                let _ = app.emit("pixtral-install-done", ());
            }
            Err(err) => {
                let _ = app.emit("pixtral-install-failed", err);
            }
        }
        INSTALLING_PIXTRAL.store(false, Ordering::SeqCst);
    });

    Ok(())
}
