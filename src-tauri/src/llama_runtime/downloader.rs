use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use super::LLAMA_CPP_TAG;

pub fn download_file(url: &str, target: &Path) -> Result<(), String> {
    download_file_with_progress(url, target, |downloaded, total| {
        report_progress(target, downloaded, total);
    })
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
                    eprintln!("[llama-download] transient failure, retrying in {:?}: {}", delay, last_err);
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
    let mut response = client
        .get(url)
        .send()
        .map_err(map_reqwest_error)?;
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
    let mut file = fs::File::create(partial).map_err(|err| DownloadError::Fatal(err.to_string()))?;
    let mut downloaded = 0u64;
    let mut last_report = Instant::now();
    let mut last_reported_bytes = 0u64;
    let mut buf = [0u8; 64 * 1024];
    const REPORT_EVERY: Duration = Duration::from_millis(500);
    const REPORT_BYTES: u64 = 8 * 1024 * 1024;

    loop {
        let n = response
            .read(&mut buf)
            .map_err(map_io_error)?;
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
    fs::create_dir_all(bin_dir).map_err(|e| e.to_string())?;

    let main_zip = bin_dir.join("llama-cuda-main.zip");
    let cudart_zip = bin_dir.join("llama-cuda-runtime.zip");

    eprintln!("[llama-runtime] LLAMA_CPP_TAG={LLAMA_CPP_TAG}");
    let main_url = llama_binary_url();
    let cudart_url = llama_cudart_url();
    download_file(&main_url, &main_zip)?;
    download_file(&cudart_url, &cudart_zip)?;

    extract_zip(&main_zip, bin_dir)?;
    extract_zip(&cudart_zip, bin_dir)?;

    let _ = fs::remove_file(&main_zip);
    let _ = fs::remove_file(&cudart_zip);

    flatten_extract(bin_dir);

    if !bin_dir.join("llama-server.exe").exists() {
        return Err("llama-server.exe not found after extract".to_string());
    }
    Ok(())
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
