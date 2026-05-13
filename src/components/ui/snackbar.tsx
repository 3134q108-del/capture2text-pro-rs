import * as React from "react";
import { cn } from "@/lib/utils";

export type SnackbarTone = "success" | "error" | "warning" | "info";

export type SnackbarMessage = {
  id: number;
  tone: SnackbarTone;
  text: string;
  duration?: number;
};

type SnackbarContextValue = {
  show: (tone: SnackbarTone, text: string, duration?: number) => void;
};

const SnackbarContext = React.createContext<SnackbarContextValue | null>(null);

export function useSnackbar() {
  const ctx = React.useContext(SnackbarContext);
  if (!ctx) {
    throw new Error("useSnackbar must be inside <SnackbarProvider>");
  }
  return ctx;
}

const ICON: Record<SnackbarTone, string> = {
  success: "✓",
  error: "✕",
  warning: "!",
  info: "i",
};

const ICON_COLOR: Record<SnackbarTone, string> = {
  success: "text-green-400",
  error: "text-red-400",
  warning: "text-yellow-400",
  info: "text-blue-400",
};

export function SnackbarProvider({ children }: { children: React.ReactNode }) {
  const [message, setMessage] = React.useState<SnackbarMessage | null>(null);
  const [hiding, setHiding] = React.useState(false);
  const timerRef = React.useRef<number | null>(null);
  const hideTimerRef = React.useRef<number | null>(null);
  const idRef = React.useRef(0);

  const clearTimer = React.useCallback(() => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    if (hideTimerRef.current !== null) {
      window.clearTimeout(hideTimerRef.current);
      hideTimerRef.current = null;
    }
  }, []);

  const hide = React.useCallback(() => {
    clearTimer();
    setHiding(true);
    hideTimerRef.current = window.setTimeout(() => {
      setMessage(null);
      setHiding(false);
      hideTimerRef.current = null;
    }, 200);
  }, [clearTimer]);

  const show = React.useCallback((tone: SnackbarTone, text: string, duration = 1500) => {
    clearTimer();
    setHiding(false);
    idRef.current += 1;
    const id = idRef.current;
    setMessage({ id, tone, text, duration });
    if (duration > 0) {
      timerRef.current = window.setTimeout(() => {
        setHiding(true);
        hideTimerRef.current = window.setTimeout(() => {
          setMessage((cur) => (cur?.id === id ? null : cur));
          setHiding(false);
          hideTimerRef.current = null;
        }, 200);
        timerRef.current = null;
      }, duration);
    }
  }, [clearTimer]);

  React.useEffect(() => () => clearTimer(), [clearTimer]);

  return (
    <SnackbarContext.Provider value={{ show }}>
      {children}
      <style>
        {`
        @keyframes snackbar-in {
          from { opacity: 0; transform: translate(-50%, 20px); }
          to { opacity: 1; transform: translate(-50%, 0); }
        }
        @keyframes snackbar-out {
          from { opacity: 1; transform: translate(-50%, 0); }
          to { opacity: 0; transform: translate(-50%, 20px); }
        }
        .animate-snackbar-in { animation: snackbar-in 200ms ease-out; }
        .animate-snackbar-out { animation: snackbar-out 200ms ease-in forwards; }
        `}
      </style>
      {message ? (
        <div
          className={cn(
            "pointer-events-auto fixed bottom-6 left-1/2 z-50 flex min-w-72 max-w-xl -translate-x-1/2 items-center gap-3 rounded-md bg-slate-800 px-5 py-3 text-sm text-white shadow-lg",
            hiding ? "animate-snackbar-out" : "animate-snackbar-in",
          )}
          role="status"
          aria-live="polite"
        >
          <span className={cn("text-base", ICON_COLOR[message.tone])}>{ICON[message.tone]}</span>
          <span className="flex-1 break-words">{message.text}</span>
          <button
            type="button"
            className="text-white/70 hover:text-white"
            onClick={hide}
            aria-label="關閉"
          >
            ×
          </button>
        </div>
      ) : null}
    </SnackbarContext.Provider>
  );
}
