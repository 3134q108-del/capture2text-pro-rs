use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, IDYES, MB_ICONINFORMATION, MB_ICONQUESTION, MB_ICONWARNING, MB_OK, MB_YESNO,
    MESSAGEBOX_STYLE,
};

use crate::vlm;

const POLL_INTERVAL_MS: u64 = 1_000;
const POLL_MAX_MS: u64 = 60_000;

fn msgbox(text: &str, title: &str, style: MESSAGEBOX_STYLE) -> i32 {
    let text_w = HSTRING::from(text);
    let title_w = HSTRING::from(title);
    unsafe {
        MessageBoxW(
            HWND::default(),
            PCWSTR(text_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            style,
        )
        .0
    }
}

/// 啟動時健檢失敗（OllamaDown）才呼叫。
pub fn prompt_and_launch() {
    thread::spawn(|| {
        let answer = msgbox(
            "未偵測到 Ollama 服務。\n\n要自動啟動 Ollama 嗎？\n（啟動後會常駐系統列，可從 Ollama 的 tray icon 關閉）",
            "Capture2Text - Ollama 未啟動",
            MB_YESNO | MB_ICONQUESTION,
        );
        if answer != IDYES.0 {
            eprintln!("[ollama-boot] user declined launch");
            return;
        }

        if let Err(err) = spawn_ollama() {
            eprintln!("[ollama-boot] spawn failed: {err}");
            msgbox(
                &format!("啟動 Ollama 失敗：{err}\n\n請手動開啟 Ollama。"),
                "Capture2Text - 啟動失敗",
                MB_OK | MB_ICONWARNING,
            );
            return;
        }

        eprintln!("[ollama-boot] spawned, polling health...");
        let started = Instant::now();
        let mut healthy = false;
        while started.elapsed() < Duration::from_millis(POLL_MAX_MS) {
            thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
            if matches!(vlm::check_health(), vlm::HealthStatus::Healthy) {
                healthy = true;
                break;
            }
        }

        if healthy {
            eprintln!("[ollama-boot] ready in {}ms", started.elapsed().as_millis());
            msgbox(
                "Ollama 已啟動並連線成功。\n\n可以開始使用 Win+Q / Win+W / Win+E 截圖。",
                "Capture2Text - Ollama 就緒",
                MB_OK | MB_ICONINFORMATION,
            );
            vlm::warmup();
        } else {
            eprintln!("[ollama-boot] timeout after {}ms", POLL_MAX_MS);
            msgbox(
                "Ollama 啟動後仍無法連線（60 秒 timeout）。\n\n請手動確認 Ollama 是否正常。",
                "Capture2Text - Ollama 連線逾時",
                MB_OK | MB_ICONWARNING,
            );
        }
    });
}

fn spawn_ollama() -> Result<(), String> {
    let candidates = candidate_paths();
    let mut last_err: Option<String> = None;
    for path in &candidates {
        if Path::new(path).exists() {
            eprintln!("[ollama-boot] launching: {path}");
            match launch_detached(path, true) {
                Ok(()) => return Ok(()),
                Err(err) => {
                    eprintln!("[ollama-boot] launch failed: {err}");
                    last_err = Some(err);
                }
            }
        }
    }

    eprintln!("[ollama-boot] no GUI app launched, try 'ollama serve' on PATH");
    match launch_detached_cli_serve() {
        Ok(()) => Ok(()),
        Err(err) => {
            if let Some(last_gui_err) = last_err {
                Err(format!("{err}; gui launch error: {last_gui_err}"))
            } else {
                Err(err)
            }
        }
    }
}

fn candidate_paths() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(local_app) = std::env::var("LOCALAPPDATA") {
        out.push(format!(r"{local_app}\Programs\Ollama\ollama app.exe"));
        out.push(format!(r"{local_app}\Programs\Ollama\Ollama.exe"));
    }
    out.push(r"C:\Program Files\Ollama\ollama app.exe".to_string());
    out.push(r"C:\Program Files\Ollama\Ollama.exe".to_string());
    out
}

fn launch_detached(path: &str, is_gui: bool) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    const DETACHED_PROCESS: u32 = 0x00000008;

    let mut cmd = Command::new(path);
    if !is_gui {
        cmd.arg("serve");
    }
    cmd.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS);
    cmd.spawn().map(|_| ()).map_err(|err| err.to_string())
}

fn launch_detached_cli_serve() -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    const DETACHED_PROCESS: u32 = 0x00000008;

    Command::new("ollama")
        .arg("serve")
        .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
        .spawn()
        .map(|_| ())
        .map_err(|err| err.to_string())
}
