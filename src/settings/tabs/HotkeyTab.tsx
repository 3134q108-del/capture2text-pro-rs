import { useEffect, useMemo, useState } from "react";
import {
  Button,
  FormField,
  KeyCapture,
  type KeyCaptureBinding,
  Section,
  SectionBody,
  SectionHeader,
  StatusText,
  Toggle,
} from "@/components/ui";
import { disable, enable, isEnabled } from "@/services/autostart";
import {
  getHotkeyConfig,
  resetHotkeyDefault,
  setHotkeyConfig,
  type HotkeyConfig,
} from "@/services/hotkey";

type RowKey = "hotkey_q" | "hotkey_w" | "hotkey_e";

const DESCRIPTIONS: Record<RowKey, { title: string; body: string[] }> = {
  hotkey_q: {
    title: "自由框選 OCR",
    body: [
      "拖框選擇任意螢幕範圍。適合多行段落、表格、複雜版面。",
      "按下後游標變十字，拖動到結束位置放開即觸發 OCR。",
    ],
  },
  hotkey_w: {
    title: "短句 OCR",
    body: [
      "游標位置往右展開 750px 寬度的單行。適合單字、按鈕、選單項目、短語。",
      "不需要拖框，按下立即 OCR。",
    ],
  },
  hotkey_e: {
    title: "長句 OCR",
    body: [
      "游標位置左右各展開 750px（共 1500px）的單行。適合整句字幕、長標題、URL。",
      "不需要拖框，按下立即 OCR。",
    ],
  },
};

const DEFAULT_CONFIG: HotkeyConfig = {
  hotkey_q: { modifiers: { ctrl: false, shift: false, alt: false, win: true }, key_code: 0x51 },
  hotkey_w: { modifiers: { ctrl: false, shift: false, alt: false, win: true }, key_code: 0x57 },
  hotkey_e: { modifiers: { ctrl: false, shift: false, alt: false, win: true }, key_code: 0x45 },
};

export default function HotkeyTab() {
  const [config, setConfig] = useState<HotkeyConfig>(DEFAULT_CONFIG);
  const [loading, setLoading] = useState(true);
  const [status, setStatus] = useState("");
  const [recordingRows, setRecordingRows] = useState<Set<RowKey>>(new Set());
  const [autostart, setAutostart] = useState<boolean | null>(null);
  const [autostartUpdating, setAutostartUpdating] = useState(false);

  useEffect(() => {
    void load();
  }, []);

  useEffect(() => {
    let cancelled = false;
    void isEnabled()
      .then((value) => {
        if (!cancelled) {
          setAutostart(value);
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setAutostart(false);
          setStatus(String(error));
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const rows = useMemo(
    () =>
      (["hotkey_q", "hotkey_w", "hotkey_e"] as RowKey[]).map((key) => ({
        key,
        binding: config[key],
        ...DESCRIPTIONS[key],
      })),
    [config],
  );

  async function load() {
    setLoading(true);
    try {
      const next = await getHotkeyConfig();
      setConfig(next);
      setStatus("");
    } catch (error) {
      setStatus(String(error));
    } finally {
      setLoading(false);
    }
  }

  async function updateOne(key: RowKey, binding: KeyCaptureBinding) {
    const next = { ...config, [key]: binding };
    setConfig(next);
    try {
      await setHotkeyConfig(next);
      setStatus("快捷鍵已更新");
    } catch (error) {
      setStatus(String(error));
    }
  }

  async function resetOne(key: RowKey) {
    await updateOne(key, DEFAULT_CONFIG[key]);
  }

  async function resetAll() {
    try {
      const next = await resetHotkeyDefault();
      setConfig(next);
      setStatus("已恢復預設快捷鍵");
    } catch (error) {
      setStatus(String(error));
    }
  }

  function setRowRecording(key: RowKey, recording: boolean) {
    setRecordingRows((prev) => {
      const next = new Set(prev);
      if (recording) {
        next.add(key);
      } else {
        next.delete(key);
      }
      return next;
    });
  }

  async function onToggleAutostart(next: boolean) {
    if (autostartUpdating || autostart === null) {
      return;
    }
    setAutostartUpdating(true);
    setAutostart(next);
    try {
      if (next) {
        await enable();
      } else {
        await disable();
      }
      setStatus("開機自動啟動設定已更新");
    } catch (error) {
      const actual = await isEnabled().catch(() => !next);
      setAutostart(actual);
      setStatus(String(error));
    } finally {
      setAutostartUpdating(false);
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <Section>
        <SectionHeader
          title="快捷鍵"
          description="三組全域快捷鍵啟動 OCR 流程。觸發後自動截圖 + 識別文字 + 翻譯。"
        />
        <SectionBody>
          <div className="flex flex-col gap-4">
            {rows.map((row) => (
              <div key={row.key} className="rounded-md border border-border p-3">
                <div className="mb-2 text-sm font-semibold text-foreground">{row.title}</div>
                <div className="mb-3 space-y-1 text-sm text-muted-foreground">
                  {row.body.map((line) => (
                    <p key={line}>{line}</p>
                  ))}
                </div>
                <FormField label="按鍵組合" orientation="vertical">
                  <div className="flex flex-wrap items-center gap-2">
                    <KeyCapture
                      value={row.binding}
                      onChange={(value) => void updateOne(row.key, value)}
                      onRecordingChange={(recording) => setRowRecording(row.key, recording)}
                    />
                    {!recordingRows.has(row.key) ? (
                      <Button type="button" variant="secondary" onClick={() => void resetOne(row.key)}>
                        重設此項
                      </Button>
                    ) : null}
                  </div>
                </FormField>
              </div>
            ))}
            <div className="rounded-md border border-border p-3">
              <Toggle
                checked={autostart ?? false}
                disabled={autostart === null || autostartUpdating}
                onCheckedChange={(next) => void onToggleAutostart(next)}
                label="開機自動啟動 (僅當前使用者)"
                description="此設定僅影響目前 Windows 使用者帳號。"
              />
            </div>
          </div>
        </SectionBody>
      </Section>

      <div className="flex items-center gap-2">
        <Button type="button" variant="secondary" onClick={() => void resetAll()} state={loading ? "loading" : "content"}>
          全部重設
        </Button>
        {status ? (
          <StatusText tone={status.includes("已") ? "success" : "error"} size="sm">
            {status}
          </StatusText>
        ) : null}
      </div>
    </div>
  );
}
