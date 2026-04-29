import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { LogicalPosition, LogicalSize, getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useReducer, useRef, useState } from "react";
import {
  Banner,
  Button,
  Checkbox,
  Modal,
  ModalContent,
  ModalDescription,
  ModalFooter,
  ModalHeader,
  ModalTitle,
  Popover,
  PopoverContent,
  PopoverTrigger,
  ProgressBar,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  StatusText,
  UsageDonut,
} from "@/components/ui";

type VlmStatus = "idle" | "loading" | "success" | "error";
type TtsTarget = "original" | "translated";
type PopupFont = { family: string; size_pt: number } | null;

type VlmEventPayload = {
  source: string;
  status: "success" | "error";
  original: string;
  translated: string;
  src_lang?: string | null;
  duration_ms: number;
  error: string | null;
};

type VlmPartialEventPayload = {
  source: string;
  original: string;
  translated: string;
  src_lang?: string | null;
};

type VlmSnapshot = {
  source: string;
  status: "loading" | "success" | "error";
  original: string;
  translated: string;
  src_lang?: string | null;
  duration_ms: number;
  error: string | null;
  updated_at: number;
};

type WindowState = {
  popup_topmost: boolean;
  popup_font: PopupFont;
};

type UsageInfo = {
  tier: "F0" | "S0";
  neural_used: number;
  hd_used: number;
  neural_limit: number;
  hd_limit: number;
  month: string;
  neural_percent: number;
  hd_percent: number;
};

type SpeakingState =
  | { kind: "idle" }
  | { kind: "synthesizing"; target: TtsTarget }
  | { kind: "playing"; target: TtsTarget };

type SpeakAction =
  | { type: "START"; target: TtsTarget }
  | { type: "SYNTHESIZED"; target: TtsTarget }
  | { type: "DONE" }
  | { type: "FAIL" };

const FONT_FAMILIES = [
  "Segoe UI",
  "Microsoft JhengHei",
  "DFKai-SB",
  "PMingLiU",
  "Arial",
  "Consolas",
  "Courier New",
  "Times New Roman",
  "Verdana",
  "Tahoma",
];

const FONT_SIZES_PT = [8, 9, 10, 11, 12, 13, 14, 15, 16, 18, 20, 22, 24, 28, 32];

function assertNever(value: never): never {
  throw new Error(`unhandled action: ${JSON.stringify(value)}`);
}

function speakReducer(state: SpeakingState, action: SpeakAction): SpeakingState {
  switch (action.type) {
    case "START":
      return { kind: "synthesizing", target: action.target };
    case "SYNTHESIZED":
      if (state.kind !== "synthesizing") {
        return state;
      }
      if (state.target !== action.target) {
        return state;
      }
      return { kind: "playing", target: state.target };
    case "DONE":
    case "FAIL":
      return { kind: "idle" };
    default:
      return assertNever(action);
  }
}

function phaseForTarget(state: SpeakingState, target: TtsTarget): "idle" | "synthesizing" | "playing" {
  if (state.kind === "idle") {
    return "idle";
  }
  if (state.target !== target) {
    return "idle";
  }
  return state.kind;
}

function detectLang(text: string): "zh" | "en" {
  return /[\u4e00-\u9fff]/.test(text) ? "zh" : "en";
}

function usagePercent(info: UsageInfo | null): number {
  if (!info) {
    return 0;
  }
  return info.tier === "F0" ? info.neural_percent : Math.max(info.neural_percent, info.hd_percent);
}

function usageTone(percent: number): "green" | "yellow" | "red" {
  if (percent >= 90) {
    return "red";
  }
  if (percent >= 70) {
    return "yellow";
  }
  return "green";
}

export default function ResultView() {
  const [status, setStatus] = useState<VlmStatus>("idle");
  const [original, setOriginal] = useState("");
  const [translated, setTranslated] = useState("");
  const [srcLang, setSrcLang] = useState<string | null>(null);
  const [outputLang, setOutputLang] = useState("zh-TW");
  const [errorMsg, setErrorMsg] = useState("");
  const [speakingState, dispatchSpeaking] = useReducer(speakReducer, { kind: "idle" } as SpeakingState);
  const [originalReady, setOriginalReady] = useState(false);
  const [translatedReady, setTranslatedReady] = useState(false);
  const [isTopmost, setIsTopmost] = useState(true);
  const [popupFont, setPopupFont] = useState<PopupFont>(null);
  const [fontModalOpen, setFontModalOpen] = useState(false);
  const [fontFamilyDraft, setFontFamilyDraft] = useState("Segoe UI");
  const [fontSizeDraftPt, setFontSizeDraftPt] = useState(13);
  const [usageInfo, setUsageInfo] = useState<UsageInfo | null>(null);
  const [usageOpen, setUsageOpen] = useState(false);

  const originalReadyTimerRef = useRef<number | null>(null);
  const lastOriginalRef = useRef("");

  const showTranslated = translated.trim().length > 0 || status === "loading";
  const hasTranslatedText = translated.trim().length > 0;
  const textStyle = popupFont
    ? { fontFamily: popupFont.family, fontSize: `${popupFont.size_pt}pt` }
    : undefined;

  function clearOriginalReadyTimer() {
    if (originalReadyTimerRef.current !== null) {
      window.clearTimeout(originalReadyTimerRef.current);
      originalReadyTimerRef.current = null;
    }
  }

  function applyFinalPayload(payload: VlmEventPayload) {
    clearOriginalReadyTimer();

    if (payload.status === "success") {
      setStatus("success");
      setOriginal(payload.original);
      setTranslated(payload.translated);
      setSrcLang(payload.src_lang ?? null);
      setErrorMsg("");
      setOriginalReady(payload.original.trim().length > 0);
      setTranslatedReady(payload.translated.trim().length > 0);
      lastOriginalRef.current = payload.original;
      return;
    }

    setStatus("error");
    setErrorMsg(payload.error ?? "unknown error");
    setOriginal("");
    setTranslated("");
    setSrcLang(null);
    dispatchSpeaking({ type: "DONE" });
    setOriginalReady(false);
    setTranslatedReady(false);
    lastOriginalRef.current = "";
  }

  function applyPartialPayload(payload: VlmPartialEventPayload) {
    setStatus("loading");
    setOriginal(payload.original);
    setTranslated(payload.translated);
    setSrcLang(payload.src_lang ?? null);
    setErrorMsg("");
    setTranslatedReady(false);

    const trimmed = payload.original.trim();
    if (trimmed.length === 0) {
      clearOriginalReadyTimer();
      setOriginalReady(false);
      lastOriginalRef.current = payload.original;
      return;
    }

    if (payload.original !== lastOriginalRef.current) {
      lastOriginalRef.current = payload.original;
      setOriginalReady(false);
      clearOriginalReadyTimer();
      originalReadyTimerRef.current = window.setTimeout(() => {
        setOriginalReady(true);
        originalReadyTimerRef.current = null;
      }, 450);
    }
  }

  function applySnapshot(snapshot: VlmSnapshot) {
    clearOriginalReadyTimer();
    setOriginal(snapshot.original);
    setTranslated(snapshot.translated);
    setSrcLang(snapshot.src_lang ?? null);

    if (snapshot.status === "success") {
      setStatus("success");
      setErrorMsg("");
      setOriginalReady(snapshot.original.trim().length > 0);
      setTranslatedReady(snapshot.translated.trim().length > 0);
    } else if (snapshot.status === "error") {
      setStatus("error");
      setErrorMsg(snapshot.error ?? "unknown error");
      dispatchSpeaking({ type: "DONE" });
      setOriginalReady(false);
      setTranslatedReady(false);
    } else {
      setStatus("loading");
      setErrorMsg("");
      setOriginalReady(snapshot.original.trim().length > 0);
      setTranslatedReady(false);
    }

    lastOriginalRef.current = snapshot.original;
  }

  useEffect(() => {
    let disposed = false;
    let hasLiveEvent = false;
    let offFinal: null | (() => void) = null;
    let offPartial: null | (() => void) = null;
    let offTtsSynthesized: null | (() => void) = null;
    let offTtsDone: null | (() => void) = null;

    const setup = async () => {
      offFinal = await listen<VlmEventPayload>("vlm-result", (event) => {
        hasLiveEvent = true;
        applyFinalPayload(event.payload);
      });
      if (disposed) {
        offFinal();
        offFinal = null;
        return;
      }

      try {
        const latest = await invoke<VlmSnapshot | null>("get_latest_vlm_state");
        if (!disposed && !hasLiveEvent && latest) {
          applySnapshot(latest);
        }
      } catch {
        // ignore
      }
      if (disposed) {
        offFinal?.();
        offFinal = null;
        return;
      }

      offPartial = await listen<VlmPartialEventPayload>("vlm-result-partial", (event) => {
        hasLiveEvent = true;
        applyPartialPayload(event.payload);
      });
      if (disposed) {
        offPartial();
        offPartial = null;
        offFinal?.();
        offFinal = null;
        return;
      }

      offTtsSynthesized = await listen<{ target?: string }>("tts-synthesized", (event) => {
        const target = event.payload?.target;
        if (target === "original" || target === "translated") {
          dispatchSpeaking({ type: "SYNTHESIZED", target });
        }
      });
      if (disposed) {
        offTtsSynthesized();
        offTtsSynthesized = null;
        offPartial?.();
        offPartial = null;
        offFinal?.();
        offFinal = null;
        return;
      }

      offTtsDone = await listen("tts-done", () => {
        dispatchSpeaking({ type: "DONE" });
      });
      if (disposed) {
        offTtsDone();
        offTtsDone = null;
        offTtsSynthesized?.();
        offTtsSynthesized = null;
        offPartial?.();
        offPartial = null;
        offFinal?.();
        offFinal = null;
      }
    };

    void setup();

    return () => {
      disposed = true;
      offFinal?.();
      offPartial?.();
      offTtsSynthesized?.();
      offTtsDone?.();
    };
  }, []);

  useEffect(() => () => clearOriginalReadyTimer(), []);

  useEffect(() => {
    let disposed = false;
    let offLang: null | (() => void) = null;

    const setup = async () => {
      try {
        const current = await invoke<string>("get_output_language");
        if (!disposed) {
          setOutputLang(current);
        }
      } catch {
        // ignore
      }
      if (disposed) {
        return;
      }

      offLang = await listen<string>("output-language-changed", (event) => {
        setOutputLang(event.payload);
      });
      if (disposed) {
        offLang();
        offLang = null;
      }
    };

    void setup();

    return () => {
      disposed = true;
      offLang?.();
    };
  }, []);

  useEffect(() => {
    void refreshUsageInfo();
  }, []);

  useEffect(() => {
    let disposed = false;
    let offState: null | (() => void) = null;

    const loadWindowState = async () => {
      try {
        const state = await invoke<WindowState>("get_window_state");
        if (!disposed) {
          setIsTopmost(Boolean(state.popup_topmost));
          setPopupFont(state.popup_font ?? null);
        }
      } catch {
        // ignore
      }
    };

    const setup = async () => {
      await loadWindowState();
      if (disposed) {
        return;
      }

      offState = await listen<WindowState>("window-state-changed", (event) => {
        setIsTopmost(Boolean(event.payload.popup_topmost));
        setPopupFont(event.payload.popup_font ?? null);
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

  useEffect(() => {
    const appWindow = getCurrentWindow();
    let disposed = false;
    let offResized: null | (() => void) = null;
    let offMoved: null | (() => void) = null;
    let saveTimer: ReturnType<typeof setTimeout> | null = null;
    const geometry = {
      x: null as number | null,
      y: null as number | null,
      w: null as number | null,
      h: null as number | null,
    };

    const scheduleSave = () => {
      if (saveTimer) {
        clearTimeout(saveTimer);
      }
      saveTimer = setTimeout(async () => {
        if (disposed) {
          return;
        }
        const { x, y, w, h } = geometry;
        if (x == null || y == null || w == null || h == null) {
          return;
        }
        try {
          await invoke("save_popup_window_geometry", { x, y, w, h });
        } catch {
          // ignore
        }
      }, 300);
    };

    const updateFromMoved = async (payload: {
      toLogical: (scale: number) => { x: number; y: number };
    }) => {
      try {
        const scale = await appWindow.scaleFactor();
        const logical = new LogicalPosition(payload.toLogical(scale));
        geometry.x = Math.round(logical.x);
        geometry.y = Math.round(logical.y);
        scheduleSave();
      } catch {
        // ignore
      }
    };

    const updateFromResized = async (payload: {
      toLogical: (scale: number) => { width: number; height: number };
    }) => {
      try {
        const scale = await appWindow.scaleFactor();
        const logical = new LogicalSize(payload.toLogical(scale));
        geometry.w = Math.max(1, Math.round(logical.width));
        geometry.h = Math.max(1, Math.round(logical.height));
        scheduleSave();
      } catch {
        // ignore
      }
    };

    const seedGeometry = async () => {
      try {
        const [scale, posPhysical, sizePhysical] = await Promise.all([
          appWindow.scaleFactor(),
          appWindow.outerPosition(),
          appWindow.outerSize(),
        ]);
        const pos = new LogicalPosition(posPhysical.toLogical(scale));
        const size = new LogicalSize(sizePhysical.toLogical(scale));
        geometry.x = Math.round(pos.x);
        geometry.y = Math.round(pos.y);
        geometry.w = Math.max(1, Math.round(size.width));
        geometry.h = Math.max(1, Math.round(size.height));
      } catch {
        // ignore
      }
    };

    const setup = async () => {
      offResized = await appWindow.onResized(({ payload }) => {
        void updateFromResized(payload);
      });
      offMoved = await appWindow.onMoved(({ payload }) => {
        void updateFromMoved(payload);
      });

      if (disposed) {
        offResized?.();
        offMoved?.();
        return;
      }

      await seedGeometry();
    };

    void setup();

    return () => {
      disposed = true;
      if (saveTimer) {
        clearTimeout(saveTimer);
      }
      offResized?.();
      offMoved?.();
    };
  }, []);

  async function copy(text: string) {
    if (!text) {
      return;
    }
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      // ignore
    }
  }

  async function retranslate() {
    const text = original.trim();
    if (!text) {
      return;
    }
    try {
      setStatus("loading");
      setTranslated("");
      setErrorMsg("");
      setTranslatedReady(false);
      await invoke("retranslate", { text });
    } catch (error) {
      setStatus("error");
      setErrorMsg(String(error));
    }
  }

  function openFontModal() {
    if (popupFont) {
      setFontFamilyDraft(popupFont.family);
      setFontSizeDraftPt(popupFont.size_pt);
    } else {
      setFontFamilyDraft("Segoe UI");
      setFontSizeDraftPt(13);
    }
    setFontModalOpen(true);
  }

  async function applyFontModal() {
    try {
      await invoke("set_popup_font", { family: fontFamilyDraft, sizePt: fontSizeDraftPt });
      setPopupFont({ family: fontFamilyDraft, size_pt: fontSizeDraftPt });
      setFontModalOpen(false);
    } catch {
      // ignore
    }
  }

  async function resetFontModal() {
    try {
      await invoke("clear_popup_font");
      setPopupFont(null);
      setFontModalOpen(false);
    } catch {
      // ignore
    }
  }

  async function toggleSpeak(target: TtsTarget) {
    const content = target === "original" ? original.trim() : translated.trim();
    const currentPhase = phaseForTarget(speakingState, target);

    if (currentPhase === "playing") {
      try {
        await invoke("stop_speaking");
      } catch {
        // ignore
      }
      return;
    }

    if (!content) {
      return;
    }

    const lang = target === "translated" ? outputLang : (srcLang ?? detectLang(content));

    try {
      if (speakingState.kind !== "idle") {
        try {
          await invoke("stop_speaking");
        } catch {
          // ignore
        }
      }
      dispatchSpeaking({ type: "START", target });
      await invoke("speak", { target, text: content, lang });
    } catch (error) {
      console.warn("[speak] failed", error);
      dispatchSpeaking({ type: "FAIL" });
    }
  }

  async function handleTopmostToggle(next: boolean) {
    try {
      await invoke("set_popup_topmost", { value: next });
      setIsTopmost(next);
    } catch {
      // ignore
    }
  }

  async function onOk() {
    try {
      await invoke("write_popup_clipboard");
    } catch {
      // ignore
    }
    try {
      await invoke("hide_result_window");
    } catch {
      // ignore
    }
  }

  async function refreshUsageInfo() {
    try {
      const usage = await invoke<UsageInfo>("get_azure_usage_info");
      setUsageInfo(usage);
    } catch {
      // ignore
    }
  }

  async function onUsageOpenChange(nextOpen: boolean) {
    if (!nextOpen) {
      setUsageOpen(false);
      return;
    }
    await refreshUsageInfo();
    setUsageOpen(true);
  }

  const originalPhase = phaseForTarget(speakingState, "original");
  const translatedPhase = phaseForTarget(speakingState, "translated");
  const originalBlockedByOther = speakingState.kind !== "idle" && speakingState.target !== "original";
  const translatedBlockedByOther = speakingState.kind !== "idle" && speakingState.target !== "translated";

  const originalSpeakDisabled =
    !original.trim() ||
    !originalReady ||
    originalBlockedByOther ||
    originalPhase === "synthesizing";
  const translatedSpeakDisabled =
    !translated.trim() ||
    !translatedReady ||
    translatedBlockedByOther ||
    translatedPhase === "synthesizing";

  const originalSpeakLabel =
    originalPhase === "synthesizing"
      ? "合成中..."
      : originalPhase === "playing"
        ? "停止"
        : "Speak 原文";
  const translatedSpeakLabel =
    translatedPhase === "synthesizing"
      ? "合成中..."
      : translatedPhase === "playing"
        ? "停止"
        : "Speak 譯文";

  return (
    <div className="flex h-screen min-h-0 flex-col bg-background text-foreground">
      <div className="flex min-h-0 flex-1 flex-col gap-2 p-2">
        {status === "error" ? (
          <Banner tone="destructive" title="Error" description={errorMsg || "unknown error"} />
        ) : (
          <>
              <textarea
                className="min-h-0 flex-1 resize-none rounded-md border border-input bg-background px-3 py-2 text-sm leading-6 text-foreground placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                value={original}
                onChange={(event) => setOriginal(event.target.value)}
                placeholder={status === "idle" ? "Waiting for capture..." : ""}
                style={textStyle}
              />
              <div className="flex flex-wrap items-center justify-end gap-1.5">
                <Button
                  type="button"
                  variant={originalPhase === "playing" ? "destructive" : "secondary"}
                  state={originalPhase === "synthesizing" ? "loading" : "content"}
                  loadingContent="合成中..."
                  disabled={originalPhase === "playing" ? false : originalSpeakDisabled}
                  onClick={() => {
                    void toggleSpeak("original");
                  }}
                >
                  {originalSpeakLabel}
                </Button>
                <Button
                  type="button"
                  variant="secondary"
                  disabled={!original}
                  onClick={() => {
                    void copy(original);
                  }}
                >
                  Copy 原文
                </Button>
            </div>

            {showTranslated ? (
              <>
                <textarea
                  className="min-h-0 flex-1 resize-none rounded-md border border-input bg-background px-3 py-2 text-sm leading-6 text-foreground placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                  value={translated}
                  readOnly
                  placeholder={status === "idle" ? "Waiting for capture..." : ""}
                  style={textStyle}
                />
                <div className="flex flex-wrap items-center justify-end gap-1.5">
                  <Button
                    type="button"
                    variant="secondary"
                    disabled={!original.trim() || status === "loading"}
                    onClick={() => {
                      void retranslate();
                    }}
                  >
                    Retranslate
                  </Button>
                  <Button
                    type="button"
                    variant={translatedPhase === "playing" ? "destructive" : "secondary"}
                    state={translatedPhase === "synthesizing" ? "loading" : "content"}
                    loadingContent="合成中..."
                    disabled={translatedPhase === "playing" ? false : translatedSpeakDisabled}
                    onClick={() => {
                      void toggleSpeak("translated");
                    }}
                  >
                    {translatedSpeakLabel}
                  </Button>
                  {hasTranslatedText ? (
                    <Button
                      type="button"
                      variant="secondary"
                      disabled={!translated}
                      onClick={() => {
                        void copy(translated);
                      }}
                    >
                      Copy 譯文
                    </Button>
                  ) : null}
                </div>
              </>
            ) : null}
          </>
        )}
      </div>

      <footer className="flex items-center gap-3 border-t border-border bg-muted/20 px-4 py-3">
        <Checkbox
          size="sm"
          className="h-8 min-h-8 items-center"
          checked={isTopmost}
          onCheckedChange={(checked) => {
            void handleTopmostToggle(checked === true);
          }}
          label="Topmost"
        />

        <div className="ml-auto flex items-center gap-2">
          <Popover open={usageOpen} onOpenChange={(next) => { void onUsageOpenChange(next); }}>
            <PopoverTrigger asChild>
              <Button type="button" variant="ghost" aria-label="Azure 用量">
                <UsageDonut
                  size="md"
                  percent={usagePercent(usageInfo)}
                  tone={usageTone(usagePercent(usageInfo))}
                  aria-label="Azure 用量圖示"
                  aria-hidden="true"
                />
              </Button>
            </PopoverTrigger>
            <PopoverContent size="md">
              {usageInfo ? (
                <div className="flex flex-col gap-3">
                  <div className="text-sm font-semibold">
                    Azure 用量 {usageInfo.month ? `(${usageInfo.month})` : ""}
                  </div>
                  <UsageBar
                    label="Neural"
                    used={usageInfo.neural_used}
                    limit={usageInfo.neural_limit}
                    percent={usageInfo.neural_percent}
                  />
                  {usageInfo.tier === "S0" ? (
                    <UsageBar
                      label="HD"
                      used={usageInfo.hd_used}
                      limit={usageInfo.hd_limit}
                      percent={usageInfo.hd_percent}
                    />
                  ) : null}
                  <a
                    className="text-sm text-primary hover:underline"
                    href="https://portal.azure.com/#view/Microsoft_Azure_ProjectOxford/CognitiveServicesHub/~/SpeechServices"
                    target="_blank"
                    rel="noreferrer"
                  >
                    Azure Portal
                  </a>
                </div>
              ) : (
                <StatusText tone="info" size="sm">
                  無可用用量資料
                </StatusText>
              )}
            </PopoverContent>
          </Popover>

          <Button type="button" variant="secondary" onClick={openFontModal}>
            Font...
          </Button>
          <Button
            type="button"
            variant="primary"
            onClick={() => {
              void onOk();
            }}
          >
            OK
          </Button>
        </div>
      </footer>

      <Modal open={fontModalOpen} onOpenChange={setFontModalOpen}>
        <ModalContent size="md">
          <ModalHeader>
            <ModalTitle>Font</ModalTitle>
            <ModalDescription>設定結果視窗的字型與字級。</ModalDescription>
          </ModalHeader>

          <div className="flex flex-col gap-3">
            <div className="flex flex-col gap-2">
              <label htmlFor="font-family-select" className="text-sm font-medium text-foreground">
                Family
              </label>
              <Select value={fontFamilyDraft} onValueChange={(value) => setFontFamilyDraft(value)}>
                <SelectTrigger id="font-family-select">
                  <SelectValue placeholder="Select font family" />
                </SelectTrigger>
                <SelectContent>
                  {FONT_FAMILIES.map((family) => (
                    <SelectItem key={family} value={family}>
                      {family}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            <div className="flex flex-col gap-2">
              <label htmlFor="font-size-select" className="text-sm font-medium text-foreground">
                Size (pt)
              </label>
              <Select
                value={String(fontSizeDraftPt)}
                onValueChange={(value) => setFontSizeDraftPt(Number(value))}
              >
                <SelectTrigger id="font-size-select">
                  <SelectValue placeholder="Select size" />
                </SelectTrigger>
                <SelectContent>
                  {FONT_SIZES_PT.map((size) => (
                    <SelectItem key={size} value={String(size)}>
                      {size}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            <div
              className="rounded-md border border-border bg-muted/40 p-3 text-sm text-foreground"
              style={{ fontFamily: fontFamilyDraft, fontSize: `${fontSizeDraftPt}pt` }}
            >
              Capture2Text 字型 Preview 123
            </div>
          </div>

          <ModalFooter>
            <Button
              type="button"
              variant="primary"
              onClick={() => {
                void applyFontModal();
              }}
            >
              Apply
            </Button>
            <Button
              type="button"
              variant="secondary"
              onClick={() => {
                void resetFontModal();
              }}
            >
              Reset to default
            </Button>
            <Button type="button" variant="ghost" onClick={() => setFontModalOpen(false)}>
              Cancel
            </Button>
          </ModalFooter>
        </ModalContent>
      </Modal>
    </div>
  );
}

function UsageBar({
  label,
  used,
  limit,
  percent,
}: {
  label: string;
  used: number;
  limit: number;
  percent: number;
}) {
  return (
    <div className="flex flex-col gap-1">
      <ProgressBar
        tone={usageTone(percent)}
        value={used}
        max={limit || 1}
        label={label}
        subLabel={`${Math.max(0, Math.min(100, percent)).toFixed(1)}%`}
      />
      <StatusText tone="info" size="sm">
        {used.toLocaleString()} / {limit.toLocaleString()}
      </StatusText>
    </div>
  );
}
