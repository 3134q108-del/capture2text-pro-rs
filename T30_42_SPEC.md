# T30 + T42 · Health retry button + 驗收收尾

## T30 的 dirty-state 決策

OK/Cancel 真正回復 state 需要「autosave → commit-on-OK」架構改動，超出這階段範圍。

現狀：
- OK / Cancel 都走 `hideAndReset`（等價）
- Scenarios 內部已有 Save / Delete 按鈕做 dirty 控制（既有）
- 其他 checkbox / slider / dropdown 都 autosave（改就 persist）

這個架構足夠，**不動 OK/Cancel 邏輯**。

唯一要補：**健康檢查 Retry 按鈕**（healthWarning banner 下方）。

## T30 · Health retry 按鈕

### 修改 `src/settings/SettingsView.tsx`

原本 healthWarning banner 是：
```tsx
{healthWarning && (
  <div className="health-warning">⚠ {healthWarning.message}</div>
)}
```

改成：
```tsx
{healthWarning && (
  <div className="health-warning">
    <span>⚠ {healthWarning.message}</span>
    <button className="c2t-btn" style={{ marginLeft: 10 }} onClick={async () => {
      try {
        const code = await invoke<string>("check_ollama_health");
        if (code === "healthy") {
          setHealthWarning(null);
        } else {
          setHealthWarning({ status: code, message: `重試後仍異常：${code}` });
        }
      } catch (err) {
        setHealthWarning({ status: "error", message: String(err) });
      }
    }}>重試</button>
  </div>
)}
```

CSS 補 `.health-warning` 改成 flex：
```css
.health-warning {
  display: flex;
  align-items: center;
  justify-content: space-between;
  /* 其他屬性不變 */
}
```

## T42 · 驗收收尾

### 1. Tray icon 不顯示問題

`tray.rs::install` 裡面：
```rust
let Some(icon) = app.default_window_icon().cloned() else {
    return Ok(());
};
```
這個 early return 在沒 default icon 時整個 tray 消失。**改成 fallback 行為**：
```rust
let icon = app.default_window_icon().cloned();
let builder = TrayIconBuilder::new()
    .menu(&menu)
    .show_menu_on_left_click(false)
    .on_menu_event(/* ... */);
let builder = match icon {
    Some(i) => builder.icon(i),
    None => builder,
};
let _tray = builder.build(app)?;
```

如果 Codex 看現有 tray.rs 結構跟這不同（T40 改過），照等價邏輯調整。

### 2. 移除過度 eprintln（保留關鍵診斷）

現有很多 `eprintln!("[vlm] ...")` / `[tts-cache]` / `[tts-synth]` 等診斷 log。這些在 dev 有用，但 prod 噪音大。

**保留**：
- `[vlm] ollama health: OK / daemon not reachable / model not found`（啟動健檢用）
- `[hotkey] WH_KEYBOARD_LL installed`（啟動成功確認）
- `[shutdown]` 系列（除錯）
- `[window] CloseRequested label=... -> prevent_close + hide`
- `[window] attach_close_handler label=...`
- 錯誤路徑的 eprintln（failed / lock poisoned / ...）

**移除**：
- `[tts-cache] prefetch cache hit / miss / prefetched N bytes / prefetch failed`（cache 細節）
- `[tts-cache] cache_get/put lock poisoned` 保留（錯誤）
- `[tts-synth] acquired lock / start / runtime_ms / client_ms / synth_ms / total_ms`（計時）
- `[tts] play_mp3 started bytes= / play_mp3 finished`（播放細節）
- `[cmd] get_latest_vlm_state called`
- `[window] show_result_window called`
- `[window] show_settings_window called`
- `[window] ensure_webview_window label=... rebuilt`（保留 `missing, rebuilding` 一行）
- `[clipboard] wrote N chars`（保留 failed / init failed）

**不確定就保留**（避免誤刪）。

### 3. React console.log 清理

類似地，`src/result/ResultView.tsx` 有不少 console.log / error。保留 error / warn，移 info / debug。

### 4. 健康檢查邏輯跟 T30 healthWarning 呼應

原本 lib.rs setup 會把 `health-warning` event emit 到 settings window（現在已經有 SettingsView listener）。這個邏輯保留。

### 5. 最後手測前 restart dev

（CC 會處理，spec 不管）

## 禁動

- **不動** 業務邏輯（hotkey / capture / vlm / tts / clipboard 運作流程）
- **不動** Cargo.toml
- **不改** 編譯設定
- **小心** 不要誤刪關鍵錯誤 eprintln

## 驗證

- `cargo check` 通過
- `cargo build` 通過
- `npm build` 通過

## 回報

```
=== T30+T42 套改結果 ===
- SettingsView.tsx healthWarning 加重試按鈕
- SettingsView.css health-warning flex
- tray.rs icon fallback
- 移除 tts-cache / tts-synth / window show 診斷 eprintln N 處
- 移除 ResultView.tsx console.log N 處（保留 error）
- cargo check: <結果>
- npm build: <結果>
- UTF-8 NoBOM

VERDICT: APPROVED
```

UTF-8 NoBOM。
