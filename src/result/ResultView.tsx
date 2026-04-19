import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import "./ResultView.css";

type VlmStatus = "idle" | "loading" | "success" | "error";

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

export default function ResultView() {
  const [status, setStatus] = useState<VlmStatus>("idle");
  const [source, setSource] = useState<string>("");
  const [original, setOriginal] = useState<string>("");
  const [translated, setTranslated] = useState<string>("");
  const [durationMs, setDurationMs] = useState<number>(0);
  const [errorMsg, setErrorMsg] = useState<string>("");

  useEffect(() => {
    const unlistenFinalPromise = listen<VlmEventPayload>("vlm-result", (event) => {
      const p = event.payload;
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
    });

    const unlistenPartialPromise = listen<VlmPartialEventPayload>(
      "vlm-result-partial",
      (event) => {
        const p = event.payload;
        setSource(p.source);
        setStatus("loading");
        setOriginal(p.original);
        setTranslated(p.translated);
        setDurationMs(0);
        setErrorMsg("");
      },
    );

    return () => {
      unlistenFinalPromise.then((off) => off());
      unlistenPartialPromise.then((off) => off());
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

  async function speakText(text: string) {
    const content = text.trim();
    if (!content) return;
    try {
      await invoke("speak", { text: content, lang: detectLang(content) });
    } catch (err) {
      setStatus("error");
      setErrorMsg(String(err));
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
              <span>原文</span>
              <div className="result-section-actions">
                <button
                  className="result-btn-copy"
                  onClick={() => copy(original)}
                  disabled={!original}
                >
                  複製
                </button>
                <button
                  className="result-btn-copy"
                  onClick={() => speakText(original)}
                  disabled={!original.trim()}
                >
                  ▶ 朗讀
                </button>
                <button
                  className="result-btn-copy"
                  onClick={retranslate}
                  disabled={!original.trim()}
                >
                  重新翻譯
                </button>
              </div>
            </div>
            <textarea
              className="result-text"
              value={original}
              onChange={(event) => setOriginal(event.target.value)}
              placeholder={status === "idle" ? "等待擷取..." : ""}
            />
          </section>
          <section className="result-section">
            <div className="result-section-header">
              <span>譯文</span>
              <div className="result-section-actions">
                <button
                  className="result-btn-copy"
                  onClick={() => copy(translated)}
                  disabled={!translated}
                >
                  複製
                </button>
                <button
                  className="result-btn-copy"
                  onClick={() => speakText(translated)}
                  disabled={!translated.trim()}
                >
                  ▶ 朗讀
                </button>
              </div>
            </div>
            <textarea
              className="result-text"
              value={translated}
              readOnly
              placeholder={status === "idle" ? "等待擷取..." : ""}
            />
          </section>
        </div>
      )}
    </div>
  );
}
