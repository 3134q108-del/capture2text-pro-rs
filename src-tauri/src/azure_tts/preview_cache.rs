use std::fs;
use std::io::Write;
use std::path::PathBuf;

pub fn cache_dir() -> Result<PathBuf, String> {
    let base = dirs::data_local_dir().ok_or_else(|| "local appdata not found".to_string())?;
    Ok(base.join("Capture2TextPro").join("tts_preview_cache"))
}

pub fn cache_path(voice_id: &str) -> Result<PathBuf, String> {
    Ok(cache_dir()?.join(format!("{}.mp3", sanitize_voice_id(voice_id))))
}

pub fn read_cached(voice_id: &str) -> Option<Vec<u8>> {
    let path = match cache_path(voice_id) {
        Ok(path) => path,
        Err(err) => {
            eprintln!("[azure-tts] preview cache path failed voice={voice_id}: {err}");
            return None;
        }
    };
    match fs::read(path) {
        Ok(bytes) if !bytes.is_empty() => Some(bytes),
        Ok(_) => {
            eprintln!("[azure-tts] preview cache ignored empty file voice={voice_id}");
            None
        }
        Err(err) => {
            if err.kind() != std::io::ErrorKind::NotFound {
                eprintln!("[azure-tts] preview cache read failed voice={voice_id}: {err}");
            }
            None
        }
    }
}

pub fn write_cache(voice_id: &str, bytes: &[u8]) -> Result<(), String> {
    let path = cache_path(voice_id)?;
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

fn sanitize_voice_id(voice_id: &str) -> String {
    voice_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::sanitize_voice_id;

    #[test]
    fn preview_cache_sanitizes_windows_reserved_chars() {
        assert_eq!(
            sanitize_voice_id("en-US-Ava:DragonHDLatestNeural"),
            "en-US-Ava_DragonHDLatestNeural"
        );
    }
}
