use std::io;

pub fn init_runtime() -> io::Result<()> {
    Ok(())
}

pub fn current() -> String {
    crate::window_state::target_lang()
}

pub fn available_langs() -> Vec<String> {
    crate::languages::all()
        .iter()
        .map(|lang| lang.code.as_str().to_string())
        .collect()
}

pub fn set(lang: &str) -> io::Result<()> {
    let state = crate::window_state::get();
    crate::window_state::set_language_preferences(
        state.native_lang,
        lang.to_string(),
        state.enabled_langs,
    )
    .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;

    if let Some(app) = crate::app_handle::get() {
        use tauri::Emitter;
        let _ = app.emit("output-language-changed", current());
    }
    Ok(())
}
