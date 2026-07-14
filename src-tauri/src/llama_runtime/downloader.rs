use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::LLAMA_CPP_TAG;

const LLAMA_CPP_TAG_MARKER_FILE: &str = "llama-cpp.tag";
const LLAMA_CPP_STAGING_PREFIX: &str = ".staging-";

pub fn download_file(url: &str, target: &Path) -> Result<(), String> {
    download_file_with_progress(url, target, |downloaded, total| {
        report_progress(target, downloaded, total);
    })
}

pub fn installed_llama_cpp_tag(bin_dir: &Path) -> Result<Option<String>, String> {
    let marker = llama_cpp_tag_marker_path(bin_dir);
    let Ok(raw) = fs::read_to_string(&marker) else {
        return Ok(None);
    };
    let tag = raw.trim();
    if tag.is_empty() {
        return Ok(None);
    }
    Ok(Some(tag.to_string()))
}

pub fn llama_binary_needs_refresh(bin_dir: &Path) -> Result<bool, String> {
    let exe_exists = bin_dir.join("llama-server.exe").exists();
    if !exe_exists {
        return Ok(true);
    }
    Ok(installed_llama_cpp_tag(bin_dir)?.as_deref() != Some(LLAMA_CPP_TAG))
}

pub fn ensure_llama_binary_installed(bin_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(bin_dir).map_err(|e| e.to_string())?;
    cleanup_residual_install_artifacts(bin_dir);
    if !llama_binary_needs_refresh(bin_dir)? {
        if let Some(installed_tag) = installed_llama_cpp_tag(bin_dir)? {
            eprintln!(
                "[llama-runtime] llama.cpp binary tag matched: {installed_tag}; skip download"
            );
        }
        return Ok(());
    }

    let exe_exists = bin_dir.join("llama-server.exe").exists();
    match installed_llama_cpp_tag(bin_dir)? {
        Some(installed_tag) if exe_exists => eprintln!(
            "[llama-runtime] llama.cpp binary tag mismatch: installed={installed_tag}, expected={LLAMA_CPP_TAG}; redownloading"
        ),
        Some(installed_tag) => eprintln!(
            "[llama-runtime] llama.cpp binary marker matched {installed_tag} but llama-server.exe is missing; redownloading {LLAMA_CPP_TAG}"
        ),
        None if exe_exists => eprintln!(
            "[llama-runtime] llama.cpp binary marker missing for existing install; redownloading {LLAMA_CPP_TAG}"
        ),
        None => eprintln!("[llama-runtime] llama.cpp binary missing; downloading {LLAMA_CPP_TAG}"),
    }

    download_llama_binary(bin_dir)
}

pub fn download_file_with_progress<F>(
    url: &str,
    target: &Path,
    mut on_progress: F,
) -> Result<(), String>
where
    F: FnMut(u64, u64),
{
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let partial = partial_path(target);
    const RETRY_DELAYS: [Duration; 2] = [Duration::from_secs(1), Duration::from_secs(3)];
    let max_attempts = RETRY_DELAYS.len() + 1;
    let mut last_err = String::new();

    for attempt in 0..max_attempts {
        if attempt > 0 {
            on_progress(0, 0);
            eprintln!(
                "[llama-download] retry attempt {}/{} for {}",
                attempt + 1,
                max_attempts,
                target.display()
            );
        }

        let _ = fs::remove_file(&partial);
        match download_once(url, &partial, &mut on_progress) {
            Ok(downloaded) => {
                fs::rename(&partial, target).map_err(|err| err.to_string())?;
                eprintln!(
                    "[llama-runtime] downloaded {} bytes -> {}",
                    downloaded,
                    target.display()
                );
                return Ok(());
            }
            Err(DownloadError::Retriable(err)) => {
                last_err = err;
                if let Some(delay) = RETRY_DELAYS.get(attempt) {
                    eprintln!(
                        "[llama-download] transient failure, retrying in {:?}: {}",
                        delay, last_err
                    );
                    thread::sleep(*delay);
                    continue;
                }
                break;
            }
            Err(DownloadError::Fatal(err)) => {
                return Err(err);
            }
        }
    }

    Err(last_err)
}

enum DownloadError {
    Retriable(String),
    Fatal(String),
}

fn partial_path(target: &Path) -> PathBuf {
    if let Some(file_name) = target.file_name().and_then(|name| name.to_str()) {
        return target.with_file_name(format!("{file_name}.partial"));
    }
    target.with_extension("partial")
}

fn download_once<F>(url: &str, partial: &Path, on_progress: &mut F) -> Result<u64, DownloadError>
where
    F: FnMut(u64, u64),
{
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(3600))
        .build()
        .map_err(|err| DownloadError::Fatal(err.to_string()))?;
    let mut response = client.get(url).send().map_err(map_reqwest_error)?;
    if !response.status().is_success() {
        let status = response.status();
        if status.is_server_error() {
            return Err(DownloadError::Retriable(format!(
                "download {url} failed: status {status}"
            )));
        }
        return Err(DownloadError::Fatal(format!(
            "download {url} failed: status {status}"
        )));
    }

    let total = response.content_length().unwrap_or(0);
    let mut file =
        fs::File::create(partial).map_err(|err| DownloadError::Fatal(err.to_string()))?;
    let mut downloaded = 0u64;
    let mut last_report = Instant::now();
    let mut last_reported_bytes = 0u64;
    let mut buf = [0u8; 64 * 1024];
    const REPORT_EVERY: Duration = Duration::from_millis(500);
    const REPORT_BYTES: u64 = 8 * 1024 * 1024;

    loop {
        let n = response.read(&mut buf).map_err(map_io_error)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .map_err(|err| DownloadError::Fatal(err.to_string()))?;
        downloaded += n as u64;
        if last_report.elapsed() >= REPORT_EVERY
            || downloaded.saturating_sub(last_reported_bytes) >= REPORT_BYTES
        {
            on_progress(downloaded, total);
            last_report = Instant::now();
            last_reported_bytes = downloaded;
        }
    }

    on_progress(downloaded, total);
    Ok(downloaded)
}

fn map_reqwest_error(err: reqwest::Error) -> DownloadError {
    if err.is_timeout() || err.is_connect() {
        return DownloadError::Retriable(err.to_string());
    }
    if let Some(status) = err.status() {
        if status.is_server_error() {
            return DownloadError::Retriable(err.to_string());
        }
        return DownloadError::Fatal(err.to_string());
    }
    let message = err.to_string();
    let lower = message.to_ascii_lowercase();
    if lower.contains("connection reset")
        || lower.contains("connection aborted")
        || lower.contains("connection closed")
    {
        DownloadError::Retriable(message)
    } else {
        DownloadError::Fatal(message)
    }
}

fn map_io_error(err: std::io::Error) -> DownloadError {
    use std::io::ErrorKind;

    let retriable = matches!(
        err.kind(),
        ErrorKind::TimedOut
            | ErrorKind::ConnectionReset
            | ErrorKind::ConnectionAborted
            | ErrorKind::Interrupted
            | ErrorKind::UnexpectedEof
    );
    if retriable {
        DownloadError::Retriable(err.to_string())
    } else {
        DownloadError::Fatal(err.to_string())
    }
}

pub fn download_llama_binary(bin_dir: &Path) -> Result<(), String> {
    let staging_dir = staging_install_dir(bin_dir);
    let staging_cleanup_target = staging_dir.clone();
    remove_path_if_exists(&staging_dir)?;
    fs::create_dir_all(&staging_dir).map_err(|e| e.to_string())?;

    let result = (|| -> Result<(), String> {
        let main_zip = staging_dir.join("llama-cuda-main.zip");
        let cudart_zip = staging_dir.join("llama-cuda-runtime.zip");

        eprintln!("[llama-runtime] LLAMA_CPP_TAG={LLAMA_CPP_TAG}");
        let main_url = llama_binary_url();
        let cudart_url = llama_cudart_url();
        download_file(&main_url, &main_zip)?;
        download_file(&cudart_url, &cudart_zip)?;

        extract_zip(&main_zip, &staging_dir)?;
        extract_zip(&cudart_zip, &staging_dir)?;

        let _ = fs::remove_file(&main_zip);
        let _ = fs::remove_file(&cudart_zip);

        flatten_extract(&staging_dir);

        if !staging_dir.join("llama-server.exe").exists() {
            return Err("llama-server.exe not found after extract".to_string());
        }

        finalize_staged_llama_binary(&staging_dir, bin_dir)
    })();

    if result.is_err() {
        let _ = remove_path_if_exists(&staging_cleanup_target);
    }

    result
}

fn write_atomic_text(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let tmp_path = path.with_extension("tmp");
    let mut file = fs::File::create(&tmp_path).map_err(|e| e.to_string())?;
    file.write_all(contents.as_bytes())
        .map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);
    fs::rename(&tmp_path, path).map_err(|e| e.to_string())?;
    Ok(())
}

fn staging_install_dir(bin_dir: &Path) -> PathBuf {
    let parent = bin_dir.parent().unwrap_or(bin_dir);
    let tag = LLAMA_CPP_TAG;
    let pid = std::process::id();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    parent.join(format!("{LLAMA_CPP_STAGING_PREFIX}{tag}-{pid}-{stamp}"))
}

fn llama_cpp_tag_marker_path(bin_dir: &Path) -> PathBuf {
    bin_dir.join(LLAMA_CPP_TAG_MARKER_FILE)
}

fn staging_bin_old_path(bin_dir: &Path) -> PathBuf {
    bin_dir.with_extension("old")
}

fn promote_staged_llama_binary(staging_dir: &Path, bin_dir: &Path) -> Result<(), String> {
    promote_staged_llama_binary_with(staging_dir, bin_dir, remove_path_if_exists)
}

fn promote_staged_llama_binary_with<F>(
    staging_dir: &Path,
    bin_dir: &Path,
    mut remove_old_bin: F,
) -> Result<(), String>
where
    F: FnMut(&Path) -> Result<(), String>,
{
    let bin_old = staging_bin_old_path(bin_dir);
    if bin_old.exists() {
        remove_old_bin(&bin_old).map_err(|err| {
            eprintln!(
                "[llama-runtime] stale {} cleanup failed before switch: {}",
                bin_old.display(),
                err
            );
            "請關閉其他 instance 後重試".to_string()
        })?;
    }

    if bin_dir.exists() {
        fs::rename(bin_dir, &bin_old).map_err(|err| {
            eprintln!(
                "[llama-runtime] rename {} -> {} failed: {}",
                bin_dir.display(),
                bin_old.display(),
                err
            );
            "請關閉其他 instance 後重試".to_string()
        })?;
    }

    if let Err(err) = fs::rename(staging_dir, bin_dir) {
        eprintln!(
            "[llama-runtime] rename {} -> {} failed: {}",
            staging_dir.display(),
            bin_dir.display(),
            err
        );
        if bin_old.exists() {
            if let Err(restore_err) = fs::rename(&bin_old, bin_dir) {
                eprintln!(
                    "[llama-runtime] restore {} -> {} failed: {}",
                    bin_old.display(),
                    bin_dir.display(),
                    restore_err
                );
                return Err(format!("{}; restore failed: {}", err, restore_err));
            }
        }
        return Err(err.to_string());
    }

    if let Err(err) = write_atomic_text(&llama_cpp_tag_marker_path(bin_dir), LLAMA_CPP_TAG) {
        eprintln!(
            "[llama-runtime] write marker {} failed: {}",
            llama_cpp_tag_marker_path(bin_dir).display(),
            err
        );
        return Err(err);
    }

    let old_bin = bin_old.clone();
    thread::spawn(move || {
        if let Err(err) = remove_path_if_exists(&old_bin) {
            eprintln!(
                "[llama-runtime] background delete {} failed: {}",
                old_bin.display(),
                err
            );
        }
    });

    Ok(())
}

fn finalize_staged_llama_binary(staging_dir: &Path, bin_dir: &Path) -> Result<(), String> {
    let result = promote_staged_llama_binary(staging_dir, bin_dir);
    if result.is_err() {
        let _ = remove_path_if_exists(staging_dir);
    }
    result
}

#[cfg(test)]
fn finalize_staged_llama_binary_with<F>(
    staging_dir: &Path,
    bin_dir: &Path,
    mut promote: F,
) -> Result<(), String>
where
    F: FnMut(&Path, &Path) -> Result<(), String>,
{
    let result = promote(staging_dir, bin_dir);
    if result.is_err() {
        let _ = remove_path_if_exists(staging_dir);
    }
    result
}

fn cleanup_residual_install_artifacts(bin_dir: &Path) {
    let Some(parent) = bin_dir.parent() else {
        return;
    };

    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if !name.starts_with(LLAMA_CPP_STAGING_PREFIX) || !path.is_dir() {
            continue;
        }

        let bin_old = path.join("bin.old");
        let _ = remove_path_if_exists(&bin_old);
        let _ = remove_path_if_exists(&path);
    }
}

fn remove_path_if_exists(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }

    let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
    if metadata.is_dir() {
        fs::remove_dir_all(path).map_err(|e| e.to_string())
    } else {
        fs::remove_file(path).map_err(|e| e.to_string())
    }
}

fn extract_zip(zip_path: &Path, destination: &Path) -> Result<(), String> {
    let zip = escape_ps_single_quoted(zip_path.to_string_lossy().as_ref());
    let dst = escape_ps_single_quoted(destination.to_string_lossy().as_ref());
    let script = format!("Expand-Archive -Force -Path '{zip}' -DestinationPath '{dst}'");

    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-Command", &script])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "Expand-Archive failed for {}: {}",
            zip_path.display(),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

fn flatten_extract(bin_dir: &Path) {
    for entry in walkdir::WalkDir::new(bin_dir)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(file_name) = path.file_name() else {
            continue;
        };
        let target = bin_dir.join(file_name);
        if path == target {
            continue;
        }

        if target.exists() {
            let _ = fs::remove_file(&target);
        }

        if fs::rename(path, &target).is_err() {
            let _ = fs::copy(path, &target);
            let _ = fs::remove_file(path);
        }
    }
}

fn report_progress(target: &Path, downloaded: u64, total: u64) {
    let percent = if total > 0 {
        downloaded as f64 * 100.0 / total as f64
    } else {
        0.0
    };
    let file_name = target
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("<unknown>");

    eprintln!(
        "[llama-download] {} {:.1}% ({}/{} bytes)",
        file_name, percent, downloaded, total
    );

    if let Some(app) = crate::app_handle::get() {
        use tauri::Emitter;
        let _ = app.emit(
            "model-download-progress",
            serde_json::json!({
                "file": file_name,
                "downloaded": downloaded,
                "total": total,
                "percent": percent,
            }),
        );
    }
}

fn escape_ps_single_quoted(input: &str) -> String {
    input.replace('\'', "''")
}

fn llama_binary_url() -> String {
    format!(
        "https://github.com/ggerganov/llama.cpp/releases/download/{0}/llama-{0}-bin-win-cuda-12.4-x64.zip",
        LLAMA_CPP_TAG
    )
}

fn llama_cudart_url() -> String {
    format!(
        "https://github.com/ggerganov/llama.cpp/releases/download/{0}/cudart-llama-bin-win-cuda-12.4-x64.zip",
        LLAMA_CPP_TAG
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_bin_dir(prefix: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "capture2text-pro-{prefix}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn write_installed_marker(bin_dir: &Path, tag: &str) {
        fs::create_dir_all(bin_dir).expect("temp bin dir should be creatable");
        fs::write(bin_dir.join("llama-server.exe"), b"stub").expect("exe stub should write");
        fs::write(llama_cpp_tag_marker_path(bin_dir), tag).expect("marker should write");
    }

    fn write_staged_install(staging_dir: &Path, tag: &str, exe_bytes: &[u8]) {
        fs::create_dir_all(staging_dir).expect("staging dir should be creatable");
        fs::write(staging_dir.join("llama-server.exe"), exe_bytes)
            .expect("staged exe should write");
        fs::write(llama_cpp_tag_marker_path(staging_dir), tag).expect("staged marker should write");
    }

    #[test]
    fn llama_binary_needs_refresh_is_false_when_marker_matches() {
        let bin_dir = temp_bin_dir("match");
        write_installed_marker(&bin_dir, LLAMA_CPP_TAG);

        let needs_refresh = llama_binary_needs_refresh(&bin_dir).expect("status should load");
        assert!(!needs_refresh);
        assert_eq!(
            installed_llama_cpp_tag(&bin_dir)
                .expect("tag should load")
                .as_deref(),
            Some(LLAMA_CPP_TAG)
        );

        let _ = fs::remove_dir_all(&bin_dir);
    }

    #[test]
    fn llama_binary_needs_refresh_is_true_when_marker_mismatches() {
        let bin_dir = temp_bin_dir("mismatch");
        write_installed_marker(&bin_dir, "b8955");

        let needs_refresh = llama_binary_needs_refresh(&bin_dir).expect("status should load");
        assert!(needs_refresh);
        assert_eq!(
            installed_llama_cpp_tag(&bin_dir)
                .expect("tag should load")
                .as_deref(),
            Some("b8955")
        );

        let _ = fs::remove_dir_all(&bin_dir);
    }

    #[test]
    fn finalize_staged_install_swaps_binary_into_place() {
        let root_dir = temp_bin_dir("finalize-success");
        let bin_dir = root_dir.join("bin");
        let staging_dir = staging_install_dir(&bin_dir);
        write_installed_marker(&bin_dir, "b8955");
        write_staged_install(&staging_dir, LLAMA_CPP_TAG, b"new");

        finalize_staged_llama_binary(&staging_dir, &bin_dir).expect("staged install should swap");

        assert!(bin_dir.exists());
        assert_eq!(
            fs::read(bin_dir.join("llama-server.exe")).expect("new exe should be readable"),
            b"new"
        );
        assert_eq!(
            installed_llama_cpp_tag(&bin_dir)
                .expect("tag should load")
                .as_deref(),
            Some(LLAMA_CPP_TAG)
        );
        assert!(!staging_dir.exists());

        let _ = fs::remove_dir_all(&root_dir);
    }

    #[test]
    fn finalize_staged_install_cleans_staging_when_old_bin_cleanup_fails() {
        let root_dir = temp_bin_dir("finalize-locked");
        let bin_dir = root_dir.join("bin");
        let staging_dir = staging_install_dir(&bin_dir);
        write_installed_marker(&bin_dir, "b8955");
        write_staged_install(&staging_dir, LLAMA_CPP_TAG, b"new");

        let bin_old = staging_bin_old_path(&bin_dir);
        fs::create_dir_all(&bin_old).expect("bin.old dir should create");

        let err =
            finalize_staged_llama_binary_with(&staging_dir, &bin_dir, |staging_path, bin_path| {
                promote_staged_llama_binary_with(staging_path, bin_path, |_path| {
                    Err("locked by another instance".to_string())
                })
            })
            .expect_err("locked old bin should block the switch");
        assert_eq!(err, "請關閉其他 instance 後重試");
        assert!(bin_dir.join("llama-server.exe").exists());
        assert!(!staging_dir.exists());

        let _ = fs::remove_dir_all(&root_dir);
    }

    #[test]
    fn residual_staging_artifacts_are_removed_on_startup() {
        let root_dir = temp_bin_dir("residual-cleanup");
        let bin_dir = root_dir.join("bin");
        let staging_dir = staging_install_dir(&bin_dir);
        write_staged_install(&staging_dir, LLAMA_CPP_TAG, b"new");
        fs::write(staging_dir.join("bin.old"), b"old").expect("nested bin.old should write");

        cleanup_residual_install_artifacts(&bin_dir);

        assert!(!staging_dir.exists());
        assert!(!staging_dir.join("bin.old").exists());

        let _ = fs::remove_dir_all(&root_dir);
    }
}
