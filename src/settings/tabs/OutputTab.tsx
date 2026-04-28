import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { appLocalDataDir, join } from "@tauri-apps/api/path";
import { useEffect, useState } from "react";
import {
  Checkbox,
  FormField,
  PathPicker,
  RadioGroup,
  RadioGroupItem,
  Section,
  SectionBody,
  SectionHeader,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  StatusText,
} from "@/components/ui";

type ClipboardMode = "None" | "OriginalOnly" | "TranslatedOnly" | "Both";
type Separator = "Space" | "Tab" | "LineBreak" | "Comma" | "Semicolon" | "Pipe";

type WindowState = {
  clipboard_mode?: string;
  translate_separator?: string;
  popup_show_enabled: boolean;
  log_enabled: boolean;
  log_file_path: string;
};

const CLIPBOARD_MODES: Array<{ code: ClipboardMode; label: string }> = [
  { code: "None", label: "不複製到剪貼簿" },
  { code: "OriginalOnly", label: "只複製原文" },
  { code: "TranslatedOnly", label: "只複製譯文" },
  { code: "Both", label: "同時複製原文與譯文" },
];

const SEPARATORS: Array<{ code: Separator; label: string }> = [
  { code: "Space", label: "空格" },
  { code: "Tab", label: "Tab" },
  { code: "LineBreak", label: "換行" },
  { code: "Comma", label: "逗號" },
  { code: "Semicolon", label: "分號" },
  { code: "Pipe", label: "直線符號" },
];

function normalizeClipboardMode(value: string | undefined): ClipboardMode {
  return (
    ["None", "OriginalOnly", "TranslatedOnly", "Both"] as const
  ).includes(value as ClipboardMode)
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
  const [defaultLogPath, setDefaultLogPath] = useState<string>("");
  const [statusMsg, setStatusMsg] = useState<string>("");

  useEffect(() => {
    void refresh();
    void loadDefaultPath();
  }, []);

  useEffect(() => {
    let cancelled = false;
    let offState: undefined | (() => void);

    listen<WindowState>("window-state-changed", (event) => {
      setClipMode(normalizeClipboardMode(event.payload.clipboard_mode));
      setSeparator(normalizeSeparator(event.payload.translate_separator));
      setShowPopup(event.payload.popup_show_enabled);
      setLogEnabled(event.payload.log_enabled);
      setLogPath(event.payload.log_file_path);
    }).then((unlisten) => {
      if (cancelled) {
        unlisten();
        return;
      }
      offState = unlisten;
    });

    return () => {
      cancelled = true;
      offState?.();
    };
  }, []);

  async function loadDefaultPath() {
    try {
      const base = await appLocalDataDir();
      setDefaultLogPath(await join(base, "captures.log"));
    } catch {
      setDefaultLogPath("captures.log");
    }
  }

  async function refresh() {
    try {
      const ws = await invoke<WindowState>("get_window_state");
      setClipMode(normalizeClipboardMode(ws.clipboard_mode));
      setSeparator(normalizeSeparator(ws.translate_separator));
      setShowPopup(ws.popup_show_enabled);
      setLogEnabled(ws.log_enabled);
      setLogPath(ws.log_file_path);
      setStatusMsg("");
    } catch (error) {
      setStatusMsg(String(error));
    }
  }

  async function updateClipMode(next: ClipboardMode) {
    setClipMode(next);
    try {
      await invoke("set_clipboard_mode", { value: next });
    } catch (error) {
      setStatusMsg(String(error));
    }
  }

  async function updateSeparator(next: Separator) {
    setSeparator(next);
    try {
      await invoke("set_translate_separator", { value: next });
    } catch (error) {
      setStatusMsg(String(error));
    }
  }

  async function updateShowPopup(value: boolean) {
    setShowPopup(value);
    try {
      await invoke("set_popup_show_enabled", { value });
    } catch (error) {
      setStatusMsg(String(error));
    }
  }

  async function updateLogEnabled(value: boolean) {
    setLogEnabled(value);
    try {
      await invoke("set_log_enabled", { value });
    } catch (error) {
      setStatusMsg(String(error));
    }
  }

  async function updateLogPath(value: string) {
    setLogPath(value);
    try {
      await invoke("set_log_file_path", { value });
    } catch (error) {
      setStatusMsg(String(error));
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <Section>
        <SectionHeader title="複製到剪貼簿" />
        <SectionBody>
          <RadioGroup
            orientation="vertical"
            value={clipMode}
            onValueChange={(value) => void updateClipMode(normalizeClipboardMode(value))}
            className="gap-2"
          >
            {CLIPBOARD_MODES.map((option) => (
              <RadioGroupItem
                key={option.code}
                id={`clip-mode-${option.code}`}
                value={option.code}
                size="sm"
                label={option.label}
              />
            ))}
          </RadioGroup>

          {clipMode === "Both" ? (
            <FormField label="原文與譯文分隔符號" htmlFor="separator-select">
              <Select value={separator} onValueChange={(value) => void updateSeparator(normalizeSeparator(value))}>
                <SelectTrigger id="separator-select">
                  <SelectValue placeholder="請選擇分隔符號" />
                </SelectTrigger>
                <SelectContent>
                  {SEPARATORS.map((option) => (
                    <SelectItem key={option.code} value={option.code}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </FormField>
          ) : null}
        </SectionBody>
      </Section>

      <Section>
        <SectionHeader title="視窗行為" />
        <SectionBody>
          <Checkbox
            checked={showPopup}
            onCheckedChange={(checked) => void updateShowPopup(checked === true)}
            label="顯示結果彈窗"
            description="完成 OCR 後自動顯示結果視窗。"
          />
        </SectionBody>
      </Section>

      <Section>
        <SectionHeader title="記錄輸出" />
        <SectionBody>
          <Checkbox
            checked={logEnabled}
            onCheckedChange={(checked) => void updateLogEnabled(checked === true)}
            label="啟用記錄檔"
            description="將 OCR 結果寫入本機記錄檔。"
          />

          <PathPicker
            mode="file-save"
            label="記錄檔路徑"
            value={logPath}
            defaultPath={logPath.trim() ? logPath : defaultLogPath}
            filters={[{ name: "Log", extensions: ["log", "txt"] }]}
            buttonLabel="選擇路徑"
            placeholder="尚未設定"
            disabled={!logEnabled}
            onChange={(path) => {
              void updateLogPath(path);
            }}
            onPickError={(message) => setStatusMsg(message)}
          />

          <StatusText tone="info" size="sm">
            變數格式: {"{timestamp}"} {"{original_text}"} {"{translated_text}"}
          </StatusText>
        </SectionBody>
      </Section>

      {statusMsg ? (
        <StatusText tone="info" size="sm">
          {statusMsg}
        </StatusText>
      ) : null}
    </div>
  );
}
