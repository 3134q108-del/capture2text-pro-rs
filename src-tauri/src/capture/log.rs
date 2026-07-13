use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

use chrono::Local;

const PERF_LOG_ROTATE_BYTES: u64 = 5 * 1024 * 1024;
static PERF_LOG_WRITE_LOCK: Mutex<()> = Mutex::new(());

pub fn append_capture(original: &str, translated: &str) {
    let state = crate::window_state::get();
    if !state.log_enabled {
        return;
    }
    if !state.save_capture_original && !state.save_capture_translated {
        return;
    }
    let path = state.log_file_path.trim().to_string();
    if path.is_empty() {
        return;
    }
    if let Some(parent) = Path::new(&path).parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            eprintln!(
                "[capture-log] create dir {} failed: {}",
                parent.display(),
                err
            );
            return;
        }
    }

    let ts = Local::now().format("%Y-%m-%d %H:%M:%S");
    let original_out = if state.save_capture_original {
        original.replace(['\t', '\n'], " ")
    } else {
        String::new()
    };
    let translated_out = if state.save_capture_translated {
        translated.replace(['\t', '\n'], " ")
    } else {
        String::new()
    };
    let line = format!("{}\t{}\t{}\n", ts, original_out, translated_out);
    append_line(Path::new(&path), &line, "capture-log");
}

pub fn append_perf_log_line(line: &str) {
    let path = crate::app_paths::data_dir().join("logs").join("perf.log");
    let line = format!("{line}\n");
    let _guard = PERF_LOG_WRITE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    append_rotating_line(&path, &line, PERF_LOG_ROTATE_BYTES, "perf-log");
}

fn append_rotating_line(path: &Path, line: &str, rotate_bytes: u64, label: &str) {
    if let Some(parent) = path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            eprintln!(
                "[{}] create dir {} failed: {}",
                label,
                parent.display(),
                err
            );
            return;
        }
    }
    if let Err(err) = rotate_if_needed(path, rotate_bytes, label) {
        eprintln!("[{}] rotate {} failed: {}", label, path.display(), err);
        return;
    }
    append_line(path, line, label);
}

fn rotate_if_needed(path: &Path, rotate_bytes: u64, label: &str) -> std::io::Result<()> {
    let Ok(metadata) = fs::metadata(path) else {
        return Ok(());
    };
    if metadata.len() <= rotate_bytes {
        return Ok(());
    }

    let rotated_path = path.with_extension("log.1");
    if let Err(err) = fs::rename(path, &rotated_path) {
        eprintln!(
            "[{}] rotate log {} -> {} failed: {}; truncating",
            label,
            path.display(),
            rotated_path.display(),
            err
        );
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;
    }
    Ok(())
}

fn append_line(path: &Path, line: &str, label: &str) {
    let mut file = match OpenOptions::new().create(true).append(true).open(path) {
        Ok(file) => file,
        Err(err) => {
            eprintln!("[{}] open {} failed: {}", label, path.display(), err);
            return;
        }
    };
    if let Err(err) = file.write_all(line.as_bytes()) {
        eprintln!("[{}] write {} failed: {}", label, path.display(), err);
    }
}
