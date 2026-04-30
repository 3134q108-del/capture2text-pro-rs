use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::PathBuf;

pub fn cache_dir() -> Result<PathBuf, String> {
    Ok(crate::app_paths::data_dir().join("tts_speak_cache"))
}

pub fn cache_path(voice_id: &str, text: &str, rate: f32, volume: f32) -> Result<PathBuf, String> {
    Ok(cache_dir()?.join(format!("{}.mp3", cache_key_hex(voice_id, text, rate, volume))))
}

pub fn read_cached(voice_id: &str, text: &str, rate: f32, volume: f32) -> Option<Vec<u8>> {
    let path = match cache_path(voice_id, text, rate, volume) {
        Ok(path) => path,
        Err(err) => {
            eprintln!("[azure-tts] speak cache path failed voice={voice_id}: {err}");
            return None;
        }
    };
    match fs::read(path) {
        Ok(bytes) if !bytes.is_empty() => Some(bytes),
        Ok(_) => {
            eprintln!("[azure-tts] speak cache ignored empty file voice={voice_id}");
            None
        }
        Err(err) => {
            if err.kind() != std::io::ErrorKind::NotFound {
                eprintln!("[azure-tts] speak cache read failed voice={voice_id}: {err}");
            }
            None
        }
    }
}

pub fn write_cache(voice_id: &str, text: &str, rate: f32, volume: f32, bytes: &[u8]) -> Result<(), String> {
    let path = cache_path(voice_id, text, rate, volume)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let tmp_path = path.with_extension(format!("mp3.tmp.{}", std::process::id()));
    let mut file = fs::File::create(&tmp_path).map_err(|err| err.to_string())?;
    file.write_all(bytes).map_err(|err| err.to_string())?;
    file.sync_all().map_err(|err| err.to_string())?;
    drop(file);
    atomic_replace(&tmp_path, &path).map_err(|err| {
        let _ = fs::remove_file(&tmp_path);
        err.to_string()
    })?;
    Ok(())
}

fn cache_key_hex(voice_id: &str, text: &str, rate: f32, volume: f32) -> String {
    let mut hasher = DefaultHasher::new();
    voice_id.hash(&mut hasher);
    "|".hash(&mut hasher);
    escape_xml(text).hash(&mut hasher);
    "|".hash(&mut hasher);
    rate.to_bits().hash(&mut hasher);
    "|".hash(&mut hasher);
    volume.to_bits().hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn escape_xml(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(windows)]
fn atomic_replace(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let from_wide: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to_wide: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        MoveFileExW(
            PCWSTR(from_wide.as_ptr()),
            PCWSTR(to_wide.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err.to_string()))
    }
}

#[cfg(not(windows))]
fn atomic_replace(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    fs::rename(from, to)
}

#[cfg(test)]
mod tests {
    use super::cache_key_hex;

    #[test]
    fn cache_key_uses_rate_bits() {
        let a = cache_key_hex("zh-TW-HsiaoChenNeural", "hello", 1.0, 1.0);
        let b = cache_key_hex("zh-TW-HsiaoChenNeural", "hello", 1.1, 1.0);
        assert_ne!(a, b);
    }

    #[test]
    fn cache_key_uses_escaped_text() {
        let a = cache_key_hex("zh-TW-HsiaoChenNeural", "a&b", 1.0, 1.0);
        let b = cache_key_hex("zh-TW-HsiaoChenNeural", "a&amp;b", 1.0, 1.0);
        assert_ne!(a, b);
    }

    #[test]
    fn cache_key_uses_volume_bits() {
        let a = cache_key_hex("zh-TW-HsiaoChenNeural", "hello", 1.0, 1.0);
        let b = cache_key_hex("zh-TW-HsiaoChenNeural", "hello", 1.0, 1.1);
        assert_ne!(a, b);
    }
}
