use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use chrono::Local;

pub fn append_capture(original: &str, translated: &str) {
    let state = crate::window_state::get();
    if !state.log_enabled {
        return;
    }
    let path = state.log_file_path.trim().to_string();
    if path.is_empty() {
        return;
    }
    if let Some(parent) = Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let ts = Local::now().format("%Y-%m-%d %H:%M:%S");
    let line = format!(
        "{}\t{}\t{}\n",
        ts,
        original.replace('\t', " ").replace('\n', " "),
        translated.replace('\t', " ").replace('\n', " ")
    );
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(line.as_bytes());
    }
}
