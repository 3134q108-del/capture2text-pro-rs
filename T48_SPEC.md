# T48 · Ollama 背景啟動（CLI serve）+ UI spacing tokens 統整

## 目標

1. **Ollama 啟動改走 `ollama.exe serve`**（CLI daemon，完全背景，不開 UI 視窗）
   - 目前 `ollama_boot::spawn_ollama` 優先 `ollama app.exe`（GUI 管理 UI），導致每次啟動都跳管理視窗
   - 改成優先 CLI binary，GUI fallback
2. **UI spacing 統一 tokens**：在 `tokens.css` 新增 spacing / radius 變數，SettingsView.css + ResultView.css 改用 var 取代 hardcoded px

## 鎖死（MUST）

### 1. `src-tauri/src/ollama_boot.rs`：priority 改 CLI serve

改 `candidate_paths` 和 `spawn_ollama`：

```rust
/// 回傳 (path, is_gui) pair list，按優先順序
fn candidate_launchers() -> Vec<(String, bool)> {
    let mut out = Vec::new();
    if let Ok(local_app) = std::env::var("LOCALAPPDATA") {
        // 優先：CLI serve（完全背景）
        out.push((format!(r"{local_app}\Programs\Ollama\ollama.exe"), false));
        // fallback：GUI tray app
        out.push((format!(r"{local_app}\Programs\Ollama\ollama app.exe"), true));
    }
    // 64-bit Program Files 安裝位置
    out.push((r"C:\Program Files\Ollama\ollama.exe".to_string(), false));
    out.push((r"C:\Program Files\Ollama\ollama app.exe".to_string(), true));
    out
}

fn spawn_ollama() -> Result<(), String> {
    for (path, is_gui) in &candidate_launchers() {
        if std::path::Path::new(path).exists() {
            eprintln!("[ollama-boot] launching: {path} (gui={is_gui})");
            return launch_detached(path, *is_gui);
        }
    }
    // fallback：PATH 上的 ollama serve
    eprintln!("[ollama-boot] no local install found, try 'ollama serve' on PATH");
    launch_detached_cli_serve()
}
```

`launch_detached` 維持原本行為（`is_gui=true` 直接跑，`is_gui=false` 加 `serve` 參數），**同時確保 `CREATE_NO_WINDOW | DETACHED_PROCESS` 兩個 flag**都有 — 這樣 CLI serve 才真的完全背景無 console 視窗。

### 2. `src/styles/tokens.css`：新增 spacing / radius 變數

在既有 `:root {}` 區塊內補：

```css
/* Spacing scale（Segoe UI 13px baseline, 4px grid）*/
--c2t-space-xs: 4px;
--c2t-space-sm: 6px;
--c2t-space-md: 10px;
--c2t-space-lg: 14px;
--c2t-space-xl: 20px;

/* Radius（Windows 11 Fluent 常見值）*/
--c2t-radius: 4px;
--c2t-radius-lg: 8px;

/* 文字次要顏色 */
--c2t-text-muted: #606060;
```

若 `--c2t-text-muted` / `--c2t-radius` 已存在就跳過那條，保持現狀。

### 3. `src/settings/SettingsView.css`：用 tokens 取代 hardcoded px

把所有 hardcoded `padding`, `margin`, `gap`, `border-radius` 的 px 值，**對應換成** token：

| 現有 | 對應 token |
|------|-----------|
| `2px`, `3px`, `4px` | `var(--c2t-space-xs)` |
| `6px`, `7px`, `8px` | `var(--c2t-space-sm)` |
| `10px`, `12px` | `var(--c2t-space-md)` |
| `14px`, `16px` | `var(--c2t-space-lg)` |
| `18px`, `20px` | `var(--c2t-space-xl)` |
| `border-radius: 4px` | `var(--c2t-radius)` |
| `border-radius: 8px` | `var(--c2t-radius-lg)` |
| `color: #606060` / `#707070` | `var(--c2t-text-muted)` |

**非 spacing 數值保留**（`width`, `height`, `min-width`, `grid-template-columns`, `font-size` 等不套 spacing token）。

目標是所有 spacing 從 5 個 token 取值，不再散落 px。每個 section / card 的 padding 一致、每個 label-input gap 一致、每個 row gap 一致。

### 4. `src/result/ResultView.css`：同樣套 tokens

把 hardcoded spacing px 依 §3 表對應換成 token。

特別注意 ResultView 的 margin / padding 可能跟 popup 尺寸約束有關（661x371），換完視覺上若破版，**改成最接近的 token 值**（例如原本 7px 若選 xs=4 太小選 sm=6 太大，取 sm）。

### 5. 驗證視覺完整性

套改完後**不**要求 dev 起來測 UI；但 `npm build` 必須過（CSS syntax 正確）。

手測等 user 自己驗收（dev 在跑中，CSS 有 hot-reload 應該即時反映；Rust 改動 `ollama_boot.rs` 需 Rust 重編譯）。

## 禁動

- **不動** ResultView 的 layout 結構（只改 CSS 數值，不動 JSX 或 component）
- **不動** Rust 其他模組
- **不刪** 既有 `--c2t-*` token（可擴充）
- **不加** 顏色 token 除了 `--c2t-text-muted`（配色不是這輪）

## 驗證

- `cargo check` + `cargo build` 通過（Rust 改動）
- `npm build` 通過（CSS 改動）
- UTF-8 NoBOM

## 回報

```
=== T48 套改結果 ===
- ollama_boot.rs: candidate_launchers 改 CLI 優先 GUI fallback
- tokens.css: 加 5 個 spacing + 2 個 radius + text-muted
- SettingsView.css: 約 N 處 px 改 token
- ResultView.css: 約 M 處 px 改 token
- cargo check: <結果>
- npm build: <結果>
- UTF-8 NoBOM

VERDICT: APPROVED
```

**直接套改**。UTF-8 NoBOM。
