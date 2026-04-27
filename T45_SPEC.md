# T45 · 啟動時偵測並自動啟動 Ollama

## 目標

App 啟動健檢若 `HealthStatus::OllamaDown`：
1. 跳原生 MessageBox「未偵測到 Ollama 服務。要自動啟動嗎？」（Yes / No）
2. 按 Yes → 嘗試啟動 Ollama（優先 `ollama app.exe` GUI tray，fallback `ollama serve`）
3. Poll health 每 1s 最多 60s 等 daemon ready
4. 跳第二個 MessageBox 通知結果：「Ollama 已啟動並連線成功」或「啟動失敗，請手動開啟 Ollama」
5. 啟動成功後自動 call `vlm::warmup()`

## 鎖死（MUST）

### 1. `src-tauri/Cargo.toml`

`windows` crate 已有，**要加 feature**：
```toml
windows = { version = "0.58", features = [
  "Win32_Foundation",
  "Win32_Graphics_Gdi",
  "Win32_System_LibraryLoader",
  "Win32_UI_HiDpi",
  "Win32_UI_WindowsAndMessaging",
  "Win32_UI_Input_KeyboardAndMouse",
  "Win32_System_Threading",
] }
```
→ 確認 `Win32_UI_WindowsAndMessaging` 已含（`MessageBoxW` 在此 feature），如果沒有要加。

### 2. 新模組 `src-tauri/src/ollama_boot.rs`

```rust
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, IDYES,
    MB_YESNO, MB_OK, MB_ICONQUESTION, MB_ICONINFORMATION, MB_ICONWARNING,
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
        ).0
    }
}

/// 啟動時健檢失敗 → 走這條；非 OllamaDown 不呼叫
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
    // 優先 GUI tray app，fallback CLI serve
    let candidates = candidate_paths();
    for path in &candidates {
        if std::path::Path::new(path).exists() {
            eprintln!("[ollama-boot] launching: {path}");
            return launch_detached(path, true);
        }
    }

    // fallback: PATH 上的 ollama serve
    eprintln!("[ollama-boot] no GUI app found, try 'ollama serve' on PATH");
    launch_detached_cli_serve()
}

fn candidate_paths() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(local_app) = std::env::var("LOCALAPPDATA") {
        out.push(format!(r"{local_app}\Programs\Ollama\ollama app.exe"));
        out.push(format!(r"{local_app}\Programs\Ollama\Ollama.exe"));
    }
    // 64-bit Program Files install
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
```

### 3. `src-tauri/src/lib.rs`

`mod ollama_boot;`

setup 中目前的健檢處理（貼近這段）：
```rust
match crate::vlm::check_health() {
    crate::vlm::HealthStatus::Healthy => {
        crate::vlm::warmup();
    }
    crate::vlm::HealthStatus::OllamaDown => {
        eprintln!("[vlm] ollama down; prompt user");
        crate::ollama_boot::prompt_and_launch();
    }
    other => {
        eprintln!("[vlm] skip warmup (health {:?})", other);
        // model missing / unknown → 不自動啟動，只 emit warning event（既有）
    }
}
```

若現有邏輯是 `match` 而非 `if` / 需調整，Codex 依實際結構保持語意等價。

### 4. 測試計畫（文件化，不需跑）

手測步驟（完成後 user 跑）：
1. 先 `taskkill /F /IM "ollama app.exe" /IM ollama.exe` 關 Ollama
2. 啟動 Capture2Text Pro
3. 看到 MessageBox「未偵測到 Ollama」→ 按 Yes
4. Ollama tray icon 出現 + 等 60s 內看到「Ollama 已啟動並連線成功」
5. Win+Q 截圖正常運作

## 禁動

- **不動** 現有 health check emit / warmup 邏輯
- **不動** 其他 tab / tray / clipboard
- **不動** Cargo.toml 除了 `windows` feature 確認

## 驗證

- `cargo check` + `cargo build`
- `npm build`（本次沒動前端可跳，但還是跑確認無 regression）
- UTF-8 NoBOM

## 風險

1. **`Win32_UI_WindowsAndMessaging` feature 是否已在 windows crate**：現有 Cargo.toml 已包含此 feature（T34c 為了 layered window mouse event）。確認即可。
2. **MessageBox 在 spawn 出來的 thread 上跑**：Win32 API 允許 non-main thread 顯示 MessageBox（這個 thread 自帶 message pump），應該 ok。
3. **DETACHED_PROCESS 行為**：child process 完全獨立，app 關掉 Ollama 不會跟著關（這是 user 要的，Ollama 作為 OS-level service）。
4. **user 拒絕或啟動失敗**：app 繼續運作，只是 Win+Q 會出 connection refused error（現行行為，已有 error banner）。

## 回報

```
=== T45 套改結果 ===
- 新檔 src-tauri/src/ollama_boot.rs
- lib.rs 加 mod ollama_boot; + setup OllamaDown 分支呼叫 prompt_and_launch
- Cargo.toml windows feature 確認含 Win32_UI_WindowsAndMessaging
- cargo check: <結果>
- cargo build: <結果>
- UTF-8 NoBOM

VERDICT: APPROVED
```

**直接套改**。UTF-8 NoBOM。
