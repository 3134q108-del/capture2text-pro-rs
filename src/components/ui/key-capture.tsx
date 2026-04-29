import { useMemo, useState } from "react";
import { cn } from "@/lib/utils";
import { Button } from "./button";
import { StatusText } from "./status-text";

export type KeyCaptureModifiers = {
  ctrl: boolean;
  shift: boolean;
  alt: boolean;
  win: boolean;
};

export type KeyCaptureBinding = {
  modifiers: KeyCaptureModifiers;
  key_code: number;
};

type CaptureState = "idle" | "capturing" | "review";

type Props = {
  value: KeyCaptureBinding;
  onChange: (value: KeyCaptureBinding) => void;
  className?: string;
};

const MODIFIER_CODES = new Set([
  "ControlLeft",
  "ControlRight",
  "ShiftLeft",
  "ShiftRight",
  "AltLeft",
  "AltRight",
  "MetaLeft",
  "MetaRight",
]);

const CODE_TO_VK: Record<string, number> = {
  ...Object.fromEntries(Array.from({ length: 26 }, (_, i) => [`Key${String.fromCharCode(65 + i)}`, 0x41 + i])),
  ...Object.fromEntries(Array.from({ length: 10 }, (_, i) => [`Digit${i}`, 0x30 + i])),
  ...Object.fromEntries(Array.from({ length: 24 }, (_, i) => [`F${i + 1}`, 0x70 + i])),
  ArrowUp: 0x26,
  ArrowDown: 0x28,
  ArrowLeft: 0x25,
  ArrowRight: 0x27,
  Enter: 0x0d,
  Escape: 0x1b,
  Space: 0x20,
  Tab: 0x09,
  Backspace: 0x08,
  Delete: 0x2e,
  Insert: 0x2d,
  Home: 0x24,
  End: 0x23,
  PageUp: 0x21,
  PageDown: 0x22,
  Minus: 0xbd,
  Equal: 0xbb,
  BracketLeft: 0xdb,
  BracketRight: 0xdd,
  Backslash: 0xdc,
  Semicolon: 0xba,
  Quote: 0xde,
  Backquote: 0xc0,
  Comma: 0xbc,
  Period: 0xbe,
  Slash: 0xbf,
  Numpad0: 0x60,
  Numpad1: 0x61,
  Numpad2: 0x62,
  Numpad3: 0x63,
  Numpad4: 0x64,
  Numpad5: 0x65,
  Numpad6: 0x66,
  Numpad7: 0x67,
  Numpad8: 0x68,
  Numpad9: 0x69,
  NumpadMultiply: 0x6a,
  NumpadAdd: 0x6b,
  NumpadSubtract: 0x6d,
  NumpadDecimal: 0x6e,
  NumpadDivide: 0x6f,
};

function formatBinding(binding: KeyCaptureBinding): string {
  const parts: string[] = [];
  if (binding.modifiers.ctrl) parts.push("Ctrl");
  if (binding.modifiers.shift) parts.push("Shift");
  if (binding.modifiers.alt) parts.push("Alt");
  if (binding.modifiers.win) parts.push("Win");
  parts.push(vkLabel(binding.key_code));
  return parts.join(" + ");
}

function vkLabel(vk: number): string {
  if (vk >= 0x41 && vk <= 0x5a) return String.fromCharCode(vk);
  if (vk >= 0x30 && vk <= 0x39) return String.fromCharCode(vk);
  if (vk >= 0x70 && vk <= 0x87) return `F${vk - 0x6f}`;
  return `VK ${vk}`;
}

export function KeyCapture({ value, onChange, className }: Props) {
  const [state, setState] = useState<CaptureState>("idle");
  const [candidate, setCandidate] = useState<KeyCaptureBinding | null>(null);
  const [error, setError] = useState("");

  const display = useMemo(() => formatBinding(value), [value]);
  const candidateDisplay = useMemo(
    () => (candidate ? formatBinding(candidate) : ""),
    [candidate],
  );

  function beginCapture() {
    setError("");
    setCandidate(null);
    setState("capturing");
  }

  function cancelCapture() {
    setError("");
    setCandidate(null);
    setState("idle");
  }

  function confirmCapture() {
    if (!candidate) return;
    onChange(candidate);
    setState("idle");
    setCandidate(null);
    setError("");
  }

  function onCaptureKeyDown(event: React.KeyboardEvent<HTMLButtonElement>) {
    event.preventDefault();
    event.stopPropagation();
    if (MODIFIER_CODES.has(event.code)) {
      return;
    }
    const vk = CODE_TO_VK[event.code];
    if (!vk) {
      setError("不支援此按鍵，請選別組");
      return;
    }
    setError("");
    setCandidate({
      modifiers: {
        ctrl: event.ctrlKey,
        shift: event.shiftKey,
        alt: event.altKey,
        win: event.metaKey,
      },
      key_code: vk,
    });
    setState("review");
  }

  return (
    <div className={cn("flex flex-col gap-2", className)}>
      <div className="flex flex-wrap items-center gap-2">
        <div className="inline-flex h-8 min-w-56 items-center rounded-md border border-border bg-muted/50 px-3 text-sm text-foreground">
          {state === "capturing" ? "按下你想要的組合..." : state === "review" ? candidateDisplay : display}
        </div>
        {state === "idle" ? (
          <Button type="button" variant="secondary" size="md" onClick={beginCapture}>
            錄製
          </Button>
        ) : null}
        {state === "capturing" ? (
          <Button type="button" variant="ghost" size="md" onClick={cancelCapture}>
            取消
          </Button>
        ) : null}
        {state === "review" ? (
          <>
            <Button type="button" variant="primary" size="md" onClick={confirmCapture}>
              確認
            </Button>
            <Button type="button" variant="secondary" size="md" onClick={beginCapture}>
              重錄
            </Button>
            <Button type="button" variant="ghost" size="md" onClick={cancelCapture}>
              取消
            </Button>
          </>
        ) : null}
      </div>

      {state === "capturing" ? (
        <button
          type="button"
          aria-label="快捷鍵錄製輸入"
          className="h-10 rounded-md border border-dashed border-input bg-background px-3 text-left text-sm text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
          onKeyDown={onCaptureKeyDown}
          autoFocus
        >
          焦點已啟用，請直接按鍵盤組合
        </button>
      ) : null}

      {error ? (
        <StatusText tone="error" size="sm">
          {error}
        </StatusText>
      ) : null}
    </div>
  );
}
