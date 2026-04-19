import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { LogicalPosition, LogicalSize, getCurrentWindow } from "@tauri-apps/api/window";
import "./ResultView.css";

type VlmStatus = "idle" | "loading" | "success" | "error";
type SpeakingTarget = "original" | "translated" | null;

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

export default function ResultView() {
  const [status, setStatus] = useState<VlmStatus>("idle");
  const [source, setSource] = useState<string>("");
  const [original, setOriginal] = useState<string>("");
  const [translated, setTranslated] = useState<string>("");
  const [durationMs, setDurationMs] = useState<number>(0);
  const [errorMsg, setErrorMsg] = useState<string>("");
  const [speakingTarget, setSpeakingTarget] = useState<SpeakingTarget>(null);
  const playRequestRef = useRef(0);

  function applyFinalPayload(p: VlmEventPayload) {
    setSource(p.source);
    if (p.status === "success") {
      setStatus("success");
      setOriginal(p.original);
      setTranslated(p.translated);
      setDurationMs(p.duration_ms);
      setErrorMsg("");
    } else {
      setStatus("error");
      setErrorMsg(p.error ?? "unknown error");
      setOriginal("");
      setTranslated("");
      setDurationMs(0);
    }
  }

  function applyPartialPayload(p: VlmPartialEventPayload) {
    setSource(p.source);
    setStatus("loading");
    setOriginal(p.original);
    setTranslated(p.translated);
    setDurationMs(0);
    setErrorMsg("");
  }

  function applySnapshot(snapshot: VlmSnapshot) {
    setSource(snapshot.source);
    setOriginal(snapshot.original);
    setTranslated(snapshot.translated);
    if (snapshot.status === "success") {
      setStatus("success");
      setDurationMs(snapshot.duration_ms);
      setErrorMsg("");
    } else if (snapshot.status === "error") {
      setStatus("error");
      setDurationMs(0);
      setErrorMsg(snapshot.error ?? "unknown error");
    } else {
      setStatus("loading");
      setDurationMs(0);
      setErrorMsg("");
    }
  }

  useEffect(() => {
    let disposed = false;
    let hasLiveEvent = false;
    let offFinal: null | (() => void) = null;
    let offPartial: null | (() => void) = null;

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

      try {
        const latest = await invoke<VlmSnapshot | null>("get_latest_vlm_state");
        if (!disposed && !hasLiveEvent && latest) {
          applySnapshot(latest);
        }
      } catch {
        // ignore
      }
    };

    void setup();

    return () => {
      disposed = true;
      offFinal?.();
      offPartial?.();
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

  async function hide() {
    try {
      await invoke("hide_result_window");
    } catch {
      // ignore
    }
  }

  async function openSettings() {
    try {
      await invoke("show_settings_window");
    } catch {
      // ignore
    }
  }

  async function retranslate() {
    const text = original.trim();
    if (!text) return;
    try {
      setStatus("loading");
      setSource("Retrans");
      setTranslated("");
      setDurationMs(0);
      setErrorMsg("");
      await invoke("retranslate", { text });
    } catch (err) {
      setStatus("error");
      setErrorMsg(String(err));
      setDurationMs(0);
    }
  }

  function detectLang(text: string): "zh" | "en" {
    return /[\u4e00-\u9fff]/.test(text) ? "zh" : "en";
  }

  async function toggleSpeak(target: Exclude<SpeakingTarget, null>, text: string) {
    const content = text.trim();

    if (speakingTarget === target) {
      playRequestRef.current += 1;
      try {
        await invoke("stop_speaking");
      } catch {
        // ignore
      }
      setSpeakingTarget(null);
      return;
    }

    if (!content) return;

    const requestId = playRequestRef.current + 1;
    playRequestRef.current = requestId;

    try {
      if (speakingTarget !== null) {
        await invoke("stop_speaking");
      }
      setSpeakingTarget(target);
      await invoke("speak", { text: content, lang: detectLang(content) });
    } catch (err) {
      setStatus("error");
      setErrorMsg(String(err));
    } finally {
      if (playRequestRef.current == requestId) {
        setSpeakingTarget(null);
      }
    }
  }

  return (
    <div className="result-root">
      <header className="result-header" data-tauri-drag-region>
        <span className="result-title">
          {source ? `Capture2Text · ${source}` : "Capture2Text"}
        </span>
        <div className="result-header-actions">
          {status === "success" && (
            <span className="result-duration">{durationMs} ms</span>
          )}
          <button className="result-close" onClick={openSettings} aria-label="Settings">
            ⚙
          </button>
          <button className="result-close" onClick={hide} aria-label="Close">
            ×
          </button>
        </div>
      </header>

      {status === "error" ? (
        <div className="result-error">
          <strong>Error:</strong>
          <div>{errorMsg}</div>
        </div>
      ) : (
        <div className="result-body">
          <section className="result-section">
            <div className="result-section-header">
              <span>Original</span>
              <div className="result-section-actions">
                <button
                  className="result-btn-copy"
                  onClick={() => copy(original)}
                  disabled={!original}
                >
                  Copy
                </button>
                <button
                  className={`result-btn-copy ${speakingTarget === "original" ? "playing" : ""}`}
                  onClick={() => toggleSpeak("original", original)}
                  disabled={!original.trim()}
                >
                  {speakingTarget === "original" ? "Stop" : "Speak"}
                </button>
                <button
                  className="result-btn-copy"
                  onClick={retranslate}
                  disabled={!original.trim()}
                >
                  Retranslate
                </button>
              </div>
            </div>
            <textarea
              className="result-text"
              value={original}
              onChange={(event) => setOriginal(event.target.value)}
              placeholder={status === "idle" ? "Waiting for capture..." : ""}
            />
          </section>
          <section className="result-section">
            <div className="result-section-header">
              <span>Translated</span>
              <div className="result-section-actions">
                <button
                  className="result-btn-copy"
                  onClick={() => copy(translated)}
                  disabled={!translated}
                >
                  Copy
                </button>
                <button
                  className={`result-btn-copy ${speakingTarget === "translated" ? "playing" : ""}`}
                  onClick={() => toggleSpeak("translated", translated)}
                  disabled={!translated.trim()}
                >
                  {speakingTarget === "translated" ? "Stop" : "Speak"}
                </button>
              </div>
            </div>
            <textarea
              className="result-text"
              value={translated}
              readOnly
              placeholder={status === "idle" ? "Waiting for capture..." : ""}
            />
          </section>
        </div>
      )}
    </div>
  );
}
