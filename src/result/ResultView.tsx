import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { LogicalPosition, LogicalSize, getCurrentWindow } from "@tauri-apps/api/window";
import "./ResultView.css";

type VlmStatus = "idle" | "loading" | "success" | "error";
type SpeakingTarget = "original" | "translated" | null;
type TtsTarget = Exclude<SpeakingTarget, null>;
type CacheReadyState = { original: boolean; translated: boolean };
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
  "微軟正黑體",
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
  const [speakingTarget, setSpeakingTarget] = useState<SpeakingTarget>(null);
  const [cacheReady, setCacheReady] = useState<CacheReadyState>({ original: false, translated: false });
  const [isTopmost, setIsTopmost] = useState<boolean>(true);
  const [popupFont, setPopupFont] = useState<PopupFont>(null);
  const [fontModalOpen, setFontModalOpen] = useState<boolean>(false);
  const [fontFamilyDraft, setFontFamilyDraft] = useState<string>("Segoe UI");
  const [fontSizeDraftPt, setFontSizeDraftPt] = useState<number>(13);

  const showTranslated = translated.trim().length > 0 || status === "loading";
  const hasTranslatedText = translated.trim().length > 0;

  const textStyle = popupFont
    ? { fontFamily: popupFont.family, fontSize: `${popupFont.size_pt}pt` }
    : undefined;

  function applyFinalPayload(p: VlmEventPayload) {
    console.log("[ResultView] applyFinal", p);
    if (p.status === "success") {
      setStatus("success");
      setOriginal(p.original);
      setTranslated(p.translated);
      setErrorMsg("");
      setCacheReady({ original: false, translated: false });
    } else {
      setStatus("error");
      setErrorMsg(p.error ?? "unknown error");
      setOriginal("");
      setTranslated("");
      setCacheReady({ original: false, translated: false });
      setSpeakingTarget(null);
    }
  }

  function applyPartialPayload(p: VlmPartialEventPayload) {
    console.log("[ResultView] applyPartial", p);
    setStatus("loading");
    setOriginal(p.original);
    setTranslated(p.translated);
    setErrorMsg("");
    setCacheReady({ original: false, translated: false });
    setSpeakingTarget(null);
  }

  function applySnapshot(snapshot: VlmSnapshot) {
    console.log("[ResultView] applySnapshot", snapshot);
    setOriginal(snapshot.original);
    setTranslated(snapshot.translated);
    if (snapshot.status === "success") {
      setStatus("success");
      setErrorMsg("");
      setCacheReady({ original: false, translated: false });
    } else if (snapshot.status === "error") {
      setStatus("error");
      setErrorMsg(snapshot.error ?? "unknown error");
      setCacheReady({ original: false, translated: false });
      setSpeakingTarget(null);
    } else {
      setStatus("loading");
      setErrorMsg("");
      setCacheReady({ original: false, translated: false });
      setSpeakingTarget(null);
    }
  }

  useEffect(() => {
    console.log("[ResultView] listener-effect mount");
    let disposed = false;
    let hasLiveEvent = false;
    let offFinal: null | (() => void) = null;
    let offPartial: null | (() => void) = null;
    let offPrefetchDone: null | (() => void) = null;
    let offTtsDone: null | (() => void) = null;

    const setup = async () => {
      console.log("[ResultView] setup start");
      offFinal = await listen<VlmEventPayload>("vlm-result", (event) => {
        hasLiveEvent = true;
        applyFinalPayload(event.payload);
      });
      console.log("[ResultView] vlm-result listener registered, disposed=", disposed);
      if (disposed) {
        offFinal();
        offFinal = null;
        return;
      }

      try {
        const latest = await invoke<VlmSnapshot | null>("get_latest_vlm_state");
        console.log("[ResultView] snapshot", latest);
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
      console.log("[ResultView] vlm-result-partial listener registered, disposed=", disposed);
      if (disposed) {
        offPartial();
        offPartial = null;
        offFinal?.();
        offFinal = null;
        return;
      }

      offPrefetchDone = await listen<{ target?: string }>("tts-prefetch-done", (event) => {
        const target = event.payload?.target;
        if (target === "original" || target === "translated") {
          setCacheReady((prev) => ({ ...prev, [target]: true }));
        }
      });
      if (disposed) {
        offPrefetchDone();
        offPrefetchDone = null;
        offTtsDone?.();
        offTtsDone = null;
        offPartial?.();
        offPartial = null;
        offFinal?.();
        offFinal = null;
        return;
      }

      offTtsDone = await listen<{ target?: string }>("tts-done", (event) => {
        const target = event.payload?.target;
        if (target === "original" || target === "translated") {
          setSpeakingTarget((prev) => (prev === target ? null : prev));
        }
      });
      if (disposed) {
        offTtsDone();
        offTtsDone = null;
        offPrefetchDone?.();
        offPrefetchDone = null;
        offPartial?.();
        offPartial = null;
        offFinal?.();
        offFinal = null;
      }
    };

    void setup();

    return () => {
      console.log("[ResultView] listener-effect cleanup");
      disposed = true;
      offFinal?.();
      offPartial?.();
      offPrefetchDone?.();
      offTtsDone?.();
    };
  }, []);

  useEffect(() => {
    let disposed = false;

    const refreshCacheReady = async () => {
      if (status !== "success") return;

      const originalText = original.trim();
      const translatedText = translated.trim();

      const [originalReady, translatedReady] = await Promise.all([
        originalText
          ? invoke<boolean>("is_tts_cached", { text: originalText, lang: detectLang(originalText) })
              .catch(() => false)
          : Promise.resolve(false),
        translatedText
          ? invoke<boolean>("is_tts_cached", { text: translatedText, lang: detectLang(translatedText) })
              .catch(() => false)
          : Promise.resolve(false),
      ]);

      if (!disposed) {
        setCacheReady({ original: originalReady, translated: translatedReady });
      }
    };

    void refreshCacheReady();

    return () => {
      disposed = true;
    };
  }, [status, original, translated]);

  useEffect(() => {
    console.log("[ResultView] window-state-effect mount");
    let disposed = false;

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

    void loadWindowState();

    return () => {
      disposed = true;
    };
  }, []);

  useEffect(() => {
    console.log("[ResultView] geometry-effect mount");
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
    console.log("[toggleSpeak] enter target=", target, "current=", speakingTarget);
    const content = target === "original" ? original.trim() : translated.trim();

    if (speakingTarget === target) {
      try {
        await invoke("stop_speaking");
      } catch {
        // ignore
      }
      setSpeakingTarget(null);
      return;
    }

    if (!content) return;
    const lang = detectLang(content);

    try {
      if (speakingTarget !== null) {
        await invoke("stop_speaking");
      }
      setSpeakingTarget(target);
      await invoke("speak", { target, text: content, lang });
    } catch (err) {
      console.warn("[speak] failed", err);
      setSpeakingTarget(null);
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
                  className={`c2t-btn ${speakingTarget === "original" ? "playing" : ""}`}
                  onClick={() => {
                    void toggleSpeak("original");
                  }}
                  disabled={
                    !original.trim() ||
                    !cacheReady.original ||
                    (speakingTarget !== null && speakingTarget !== "original")
                  }
                >
                  {!cacheReady.original && original.trim()
                    ? "合成中…"
                    : speakingTarget === "original"
                      ? "Stop"
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
                    className={`c2t-btn ${speakingTarget === "translated" ? "playing" : ""}`}
                    onClick={() => {
                      void toggleSpeak("translated");
                    }}
                    disabled={
                      !translated.trim() ||
                      !cacheReady.translated ||
                      (speakingTarget !== null && speakingTarget !== "translated")
                    }
                  >
                    {!cacheReady.translated && translated.trim()
                      ? "合成中…"
                      : speakingTarget === "translated"
                        ? "Stop"
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
              Capture2Text 預覽 Preview 123
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
