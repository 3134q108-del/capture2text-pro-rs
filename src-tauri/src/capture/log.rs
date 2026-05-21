use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use chrono::Local;

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
        if let Err(err) = std::fs::create_dir_all(parent) {
            eprintln!("[capture-log] create dir {} failed: {}", parent.display(), err);
            return;
        }
    }

    let ts = Local::now().format("%Y-%m-%d %H:%M:%S");
    let original_out = if state.save_capture_original {
        original.replace('\t', " ").replace('\n', " ")
    } else {
        String::new()
    };
    let translated_out = if state.save_capture_translated {
        translated.replace('\t', " ").replace('\n', " ")
    } else {
        String::new()
    };
    let line = format!("{}\t{}\t{}\n", ts, original_out, translated_out);
    let mut file = match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(file) => file,
        Err(err) => {
            eprintln!("[capture-log] open {} failed: {}", path, err);
            return;
        }
    };
    if let Err(err) = file.write_all(line.as_bytes()) {
        eprintln!("[capture-log] write {} failed: {}", path, err);
    }
}
