import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

type ClipboardMode = "None" | "OriginalOnly" | "TranslatedOnly" | "Both";
type Separator = "Space" | "Tab" | "LineBreak" | "Comma" | "Semicolon" | "Pipe";

type WindowState = {
  clipboard_mode?: string;
  translate_separator?: string;
  popup_show_enabled: boolean;
  log_enabled: boolean;
  log_file_path: string;
};

const CLIPBOARD_MODES: { code: ClipboardMode; label: string }[] = [
  { code: "None", label: "不複製" },
  { code: "OriginalOnly", label: "只複製原文" },
  { code: "TranslatedOnly", label: "只複製譯文" },
  { code: "Both", label: "複製原文+譯文" },
];

function normalizeClipboardMode(value: string | undefined): ClipboardMode {
  return (["None", "OriginalOnly", "TranslatedOnly", "Both"] as const).includes(
    value as ClipboardMode,
  )
    ? (value as ClipboardMode)
    : "OriginalOnly";
}

function normalizeSeparator(value: string | undefined): Separator {
  return (
    ["Space", "Tab", "LineBreak", "Comma", "Semicolon", "Pipe"] as const
  ).includes(value as Separator)
    ? (value as Separator)
    : "Space";
}

export default function OutputTab() {
  const [clipMode, setClipMode] = useState<ClipboardMode>("OriginalOnly");
  const [separator, setSeparator] = useState<Separator>("Space");
  const [showPopup, setShowPopup] = useState<boolean>(true);
  const [logEnabled, setLogEnabled] = useState<boolean>(false);
  const [logPath, setLogPath] = useState<string>("");
  const [statusMsg, setStatusMsg] = useState<string>("");

  useEffect(() => {
    void refresh();
  }, []);

  useEffect(() => {
    let disposed = false;
    let offState: null | (() => void) = null;

    const setup = async () => {
      offState = await listen<WindowState>("window-state-changed", (event) => {
        setClipMode(normalizeClipboardMode(event.payload.clipboard_mode));
        setSeparator(normalizeSeparator(event.payload.translate_separator));
        setShowPopup(event.payload.popup_show_enabled);
        setLogEnabled(event.payload.log_enabled);
        setLogPath(event.payload.log_file_path);
      });
      if (disposed) {
        offState();
        offState = null;
      }
    };

    void setup();
    return () => {
      disposed = true;
      offState?.();
    };
  }, []);

  async function refresh() {
    try {
      const ws = await invoke<WindowState>("get_window_state");
      setClipMode(normalizeClipboardMode(ws.clipboard_mode));
      setSeparator(normalizeSeparator(ws.translate_separator));
      setShowPopup(ws.popup_show_enabled);
      setLogEnabled(ws.log_enabled);
      setLogPath(ws.log_file_path);
      setStatusMsg("");
    } catch (err) {
      setStatusMsg(String(err));
    }
  }

  async function updateClipMode(next: ClipboardMode) {
    setClipMode(next);
    try {
      await invoke("set_clipboard_mode", { value: next });
    } catch (err) {
      setStatusMsg(String(err));
    }
  }

  async function updateSeparator(next: Separator) {
    setSeparator(next);
    try {
      await invoke("set_translate_separator", { value: next });
    } catch (err) {
      setStatusMsg(String(err));
    }
  }

  async function updateShowPopup(value: boolean) {
    setShowPopup(value);
    try {
      await invoke("set_popup_show_enabled", { value });
    } catch (err) {
      setStatusMsg(String(err));
    }
  }

  async function updateLogEnabled(value: boolean) {
    setLogEnabled(value);
    try {
      await invoke("set_log_enabled", { value });
    } catch (err) {
      setStatusMsg(String(err));
    }
  }

  async function updateLogPath(value: string) {
    setLogPath(value);
    try {
      await invoke("set_log_file_path", { value });
    } catch (err) {
      setStatusMsg(String(err));
    }
  }

  return (
    <div className="settings-translate-root">
      <section className="settings-section">
        <h2>儲存到剪貼簿</h2>
        <div className="settings-radio-col">
          {CLIPBOARD_MODES.map((opt) => (
            <label key={opt.code}>
              <input
                type="radio"
                name="clip-mode"
                checked={clipMode === opt.code}
                onChange={() => updateClipMode(opt.code)}
              />
              {opt.label}
            </label>
          ))}
        </div>
        {clipMode === "Both" && (
          <div style={{ marginTop: 10, paddingLeft: 24 }}>
            <label>
              分隔符號
              <select
                value={separator}
                onChange={(event) =>
                  updateSeparator(event.target.value as Separator)
                }
              >
                <option value="Space">空格</option>
                <option value="Tab">Tab</option>
                <option value="LineBreak">換行</option>
                <option value="Comma">逗號</option>
                <option value="Semicolon">分號</option>
                <option value="Pipe">豎線</option>
              </select>
            </label>
          </div>
        )}
      </section>

      <section className="settings-section">
        <label className="settings-checkbox">
          <input
            type="checkbox"
            checked={showPopup}
            onChange={(event) => updateShowPopup(event.target.checked)}
          />
          顯示彈窗
        </label>
      </section>

      <section className="settings-section">
        <label className="settings-checkbox">
          <input
            type="checkbox"
            checked={logEnabled}
            onChange={(event) => updateLogEnabled(event.target.checked)}
          />
          記錄到檔案
        </label>
        <div className="settings-output-log">
          <label>
            記錄檔路徑
            <input
              type="text"
              value={logPath}
              onChange={(event) => updateLogPath(event.target.value)}
            />
          </label>
          <div className="settings-output-log-hint">
            支援樣板：{"{timestamp}"}、{"{original_text}"}、{"{translated_text}"}。
          </div>
        </div>
      </section>

      {statusMsg && <div className="settings-status">{statusMsg}</div>}
    </div>
  );
}
