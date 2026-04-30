use std::fs;
use std::path::{Path, PathBuf};

pub const APP_NAME: &str = "com.capture2text.pro";
pub const LEGACY_NAME: &str = "Capture2TextPro";

fn has_user_data(dir: &Path) -> bool {
    dir.join("scenarios.json").exists()
        || dir.join("tts_config.json").exists()
        || dir
            .join("models")
            .join("qwen3-vl-8b-instruct.Q4_K_M.gguf")
            .exists()
        || dir.join("output_lang.txt").exists()
}

pub fn data_dir() -> PathBuf {
    let local = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    let new = local.join(APP_NAME);
    let old = local.join(LEGACY_NAME);
    if has_user_data(&new) {
        return new;
    }
    if has_user_data(&old) {
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
    if !old.exists() {
        return;
    }

    if !new.exists() {
        if let Err(e) = fs::create_dir_all(&new) {
            eprintln!("[migration] create new dir failed: {e}");
            return;
        }
    }

    let entries = match fs::read_dir(&old) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("[migration] read old dir failed: {e}");
            return;
        }
    };

    let mut moved = 0;
    let mut skipped = 0;
    let mut failed = 0;

    for entry in entries.flatten() {
        let src = entry.path();
        let name = entry.file_name();
        let dst = new.join(&name);

        if dst.exists() {
            skipped += 1;
            continue;
        }

        match fs::rename(&src, &dst) {
            Ok(_) => moved += 1,
            Err(e) => {
                eprintln!("[migration] {} -> {} failed: {e}", src.display(), dst.display());
                failed += 1;
            }
        }
    }

    eprintln!("[migration] {moved} moved, {skipped} skipped, {failed} failed");

    if let Ok(mut remaining) = fs::read_dir(&old) {
        if remaining.next().is_none() {
            let _ = fs::remove_dir(&old);
        }
    }
}
