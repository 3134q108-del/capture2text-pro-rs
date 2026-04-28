import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { LogicalPosition, LogicalSize, getCurrentWindow } from "@tauri-apps/api/window";
import "./ResultView.css";

type VlmStatus = "idle" | "loading" | "success" | "error";
type TtsTarget = "original" | "translated";
type SpeakingPhase = "synthesizing" | "playing";
type SpeakingState = { target: TtsTarget; phase: SpeakingPhase } | null;
type PopupFont = { family: string; size_pt: number } | null;

type VlmEventPayload = {
  source: string;
  status: "success" | "error";
  original: string;
  translated: string;
  duration_ms: number;
  error: string | null;
};

type VlmPartialEventPayload = {
  source: string;
  original: string;
  translated: string;
};

type VlmSnapshot = {
  source: string;
  status: "loading" | "success" | "error";
  original: string;
  translated: string;
  duration_ms: number;
  error: string | null;
  updated_at: number;
};

type WindowState = {
  popup_topmost: boolean;
  popup_font: PopupFont;
};

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

export default function ResultView() {
  const [status, setStatus] = useState<VlmStatus>("idle");
  const [original, setOriginal] = useState<string>("");
  const [translated, setTranslated] = useState<string>("");
  const [errorMsg, setErrorMsg] = useState<string>("");
  const [speakingState, setSpeakingState] = useState<SpeakingState>(null);
  const [originalReady, setOriginalReady] = useState<boolean>(false);
  const [translatedReady, setTranslatedReady] = useState<boolean>(false);
  const [isTopmost, setIsTopmost] = useState<boolean>(true);
  const [popupFont, setPopupFont] = useState<PopupFont>(null);
  const [fontModalOpen, setFontModalOpen] = useState<boolean>(false);
  const [fontFamilyDraft, setFontFamilyDraft] = useState<string>("Segoe UI");
  const [fontSizeDraftPt, setFontSizeDraftPt] = useState<number>(13);
  const originalReadyTimerRef = useRef<number | null>(null);
  const lastOriginalRef = useRef<string>("");

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

  function applyFinalPayload(p: VlmEventPayload) {
    clearOriginalReadyTimer();
    if (p.status === "success") {
      setStatus("success");
      setOriginal(p.original);
      setTranslated(p.translated);
      setErrorMsg("");
      setOriginalReady(p.original.trim().length > 0);
      setTranslatedReady(p.translated.trim().length > 0);
      lastOriginalRef.current = p.original;
    } else {
      setStatus("error");
      setErrorMsg(p.error ?? "unknown error");
      setOriginal("");
      setTranslated("");
      setSpeakingState(null);
      setOriginalReady(false);
      setTranslatedReady(false);
      lastOriginalRef.current = "";
    }
  }

  function applyPartialPayload(p: VlmPartialEventPayload) {
    setStatus("loading");
    setOriginal(p.original);
    setTranslated(p.translated);
    setErrorMsg("");
    setTranslatedReady(false);

    const trimmed = p.original.trim();
    if (trimmed.length === 0) {
      clearOriginalReadyTimer();
      setOriginalReady(false);
      lastOriginalRef.current = p.original;
      return;
    }

    if (p.original !== lastOriginalRef.current) {
      lastOriginalRef.current = p.original;
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
    if (snapshot.status === "success") {
      setStatus("success");
      setErrorMsg("");
      setOriginalReady(snapshot.original.trim().length > 0);
      setTranslatedReady(snapshot.translated.trim().length > 0);
    } else if (snapshot.status === "error") {
      setStatus("error");
      setErrorMsg(snapshot.error ?? "unknown error");
      setSpeakingState(null);
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
          setSpeakingState((prev) =>
            prev !== null && prev.target === target ? { target, phase: "playing" } : prev,
          );
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
        setSpeakingState(null);
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
      if (disposed) return;
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
      if (saveTimer) clearTimeout(saveTimer);
      saveTimer = setTimeout(async () => {
        if (disposed) return;
        const { x, y, w, h } = geometry;
        if (x == null || y == null || w == null || h == null) return;
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
      if (saveTimer) clearTimeout(saveTimer);
      offResized?.();
      offMoved?.();
    };
  }, []);

  async function copy(text: string) {
    if (!text) return;
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      // ignore
    }
  }

  async function retranslate() {
    const text = original.trim();
    if (!text) return;
    try {
      setStatus("loading");
      setTranslated("");
      setErrorMsg("");
      setTranslatedReady(false);
      await invoke("retranslate", { text });
    } catch (err) {
      setStatus("error");
      setErrorMsg(String(err));
    }
  }

  function detectLang(text: string): "zh" | "en" {
    return /[\u4e00-\u9fff]/.test(text) ? "zh" : "en";
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

    if (speakingState?.target === target && speakingState.phase === "playing") {
      try {
        await invoke("stop_speaking");
      } catch {
        // ignore
      }
      return;
    }

    if (!content) return;
    const lang = detectLang(content);

    try {
      if (speakingState !== null) {
        await invoke("stop_speaking");
      }
      setSpeakingState({ target, phase: "synthesizing" });
      await invoke("speak", { target, text: content, lang });
    } catch (err) {
      console.warn("[speak] failed", err);
      setSpeakingState(null);
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

  return (
    <div className="result-root">
      <div className="result-body">
        {status === "error" ? (
          <div className="result-error">
            <strong>Error:</strong>
            <div>{errorMsg}</div>
          </div>
        ) : (
          <>
            <section className="result-section">
              <textarea
                className="result-text"
                value={original}
                onChange={(event) => setOriginal(event.target.value)}
                placeholder={status === "idle" ? "Waiting for capture..." : ""}
                style={textStyle}
              />
              <div className="result-section-toolbar">
                <button
                  className={`c2t-btn ${
                    speakingState?.target === "original" && speakingState.phase === "playing"
                      ? "playing"
                      : ""
                  }`}
                  onClick={() => {
                    void toggleSpeak("original");
                  }}
                  disabled={
                    !original.trim() ||
                    !originalReady ||
                    (speakingState !== null &&
                      (speakingState.target !== "original" || speakingState.phase === "synthesizing"))
                  }
                >
                  {speakingState?.target === "original" && speakingState.phase === "synthesizing"
                    ? "合成中..."
                    : speakingState?.target === "original" && speakingState.phase === "playing"
                      ? "停止"
                      : "Speak 原文"}
                </button>
                <button
                  className="c2t-btn"
                  onClick={() => {
                    void copy(original);
                  }}
                  disabled={!original}
                >
                  Copy 原文
                </button>
              </div>
            </section>

            {showTranslated && (
              <section className="result-section">
                <textarea
                  className="result-text"
                  value={translated}
                  readOnly
                  placeholder={status === "idle" ? "Waiting for capture..." : ""}
                  style={textStyle}
                />
                <div className="result-section-toolbar">
                  <button
                    className="c2t-btn"
                    onClick={() => {
                      void retranslate();
                    }}
                    disabled={!original.trim() || status === "loading"}
                  >
                    Retranslate
                  </button>
                  <button
                    className={`c2t-btn ${
                      speakingState?.target === "translated" && speakingState.phase === "playing"
                        ? "playing"
                        : ""
                    }`}
                    onClick={() => {
                      void toggleSpeak("translated");
                    }}
                    disabled={
                      !translated.trim() ||
                      !translatedReady ||
                      (speakingState !== null &&
                        (speakingState.target !== "translated" ||
                          speakingState.phase === "synthesizing"))
                    }
                  >
                    {speakingState?.target === "translated" && speakingState.phase === "synthesizing"
                      ? "合成中..."
                      : speakingState?.target === "translated" && speakingState.phase === "playing"
                        ? "停止"
                        : "Speak 譯文"}
                  </button>
                  {hasTranslatedText && (
                    <button
                      className="c2t-btn"
                      onClick={() => {
                        void copy(translated);
                      }}
                      disabled={!translated}
                    >
                      Copy 譯文
                    </button>
                  )}
                </div>
              </section>
            )}
          </>
        )}
      </div>

      <div className="result-controls">
        <label className="result-topmost">
          <input
            type="checkbox"
            checked={isTopmost}
            onChange={(event) => {
              void handleTopmostToggle(event.target.checked);
            }}
          />
          Topmost
        </label>

        <div className="result-controls-actions">
          <button className="c2t-btn" onClick={openFontModal}>
            Font...
          </button>
          <button
            className="c2t-btn primary"
            onClick={() => {
              void onOk();
            }}
          >
            OK
          </button>
        </div>
      </div>

      {fontModalOpen && (
        <div className="font-modal-overlay" role="dialog" aria-modal="true" aria-label="Font Picker">
          <div className="font-modal">
            <h3 className="font-modal-title">Font</h3>
            <label className="font-modal-field">
              Family
              <select
                value={fontFamilyDraft}
                onChange={(event) => {
                  setFontFamilyDraft(event.target.value);
                }}
              >
                {FONT_FAMILIES.map((family) => (
                  <option key={family} value={family}>
                    {family}
                  </option>
                ))}
              </select>
            </label>
            <label className="font-modal-field">
              Size (pt)
              <select
                value={fontSizeDraftPt}
                onChange={(event) => {
                  setFontSizeDraftPt(Number(event.target.value));
                }}
              >
                {FONT_SIZES_PT.map((size) => (
                  <option key={size} value={size}>
                    {size}
                  </option>
                ))}
              </select>
            </label>

            <div
              className="font-modal-preview"
              style={{ fontFamily: fontFamilyDraft, fontSize: `${fontSizeDraftPt}pt` }}
            >
              Capture2Text ?汗 Preview 123
            </div>

            <div className="font-modal-actions">
              <button
                className="c2t-btn primary"
                onClick={() => {
                  void applyFontModal();
                }}
              >
                Apply
              </button>
              <button
                className="c2t-btn"
                onClick={() => {
                  void resetFontModal();
                }}
              >
                Reset to default
              </button>
              <button
                className="c2t-btn"
                onClick={() => {
                  setFontModalOpen(false);
                }}
              >
                Cancel
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
