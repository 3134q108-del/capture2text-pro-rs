use crate::window_state::{self, ClipboardMode};

pub fn write_capture(original: &str, translated: &str) {
    let state = window_state::get();
    let text = match state.clipboard_mode {
        ClipboardMode::None => return,
        ClipboardMode::OriginalOnly => original.to_string(),
        ClipboardMode::TranslatedOnly => translated.to_string(),
        ClipboardMode::Both => {
            let sep = separator_char(&state.translate_separator);
            format!("{original}{sep}{translated}")
        }
    };

    if text.is_empty() {
        return;
    }

    match arboard::Clipboard::new() {
        Ok(mut cb) => {
            if let Err(err) = cb.set_text(text) {
                eprintln!("[clipboard] set_text failed: {err}");
            }
        }
        Err(err) => {
            eprintln!("[clipboard] init failed: {err}");
        }
    }
}

fn separator_char(key: &str) -> &'static str {
    match key {
        "Tab" => "\t",
        "LineBreak" => "\n",
        "Comma" => ",",
        "Semicolon" => ";",
        "Pipe" => "|",
        _ => " ",
    }
}
