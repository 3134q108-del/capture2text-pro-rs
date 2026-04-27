import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import AboutTab from "./tabs/AboutTab";
import OutputTab from "./tabs/OutputTab";
import SpeechTab from "./tabs/SpeechTab";
import TranslateTab from "./tabs/TranslateTab";
import "./SettingsView.css";

type HealthWarning = {
  status: string;
  message: string;
};

type TabKey = "translate" | "speech" | "output" | "about";

export default function SettingsView() {
  const [activeTab, setActiveTab] = useState<TabKey>("translate");
  const [healthWarning, setHealthWarning] = useState<HealthWarning | null>(null);

  useEffect(() => {
    const unlistenPromise = listen<HealthWarning>("health-warning", (event) => {
      setHealthWarning(event.payload);
    });
    const navPromise = listen<string>("settings-navigate", (event) => {
      const target = event.payload;
      if (
        target === "translate" ||
        target === "speech" ||
        target === "output" ||
        target === "about"
      ) {
        setActiveTab(target as TabKey);
      }
    });
    return () => {
      unlistenPromise.then((off) => off());
      navPromise.then((off) => off());
    };
  }, []);

  async function hideAndReset() {
    setActiveTab("translate");
    try {
      await invoke("hide_settings_window");
    } catch {
      // ignore
    }
  }

  return (
    <div className="settings-root">
      {healthWarning && (
        <div className="health-warning">
          <span>⚠ {healthWarning.message}</span>
          <button
            className="c2t-btn"
            style={{ marginLeft: 10 }}
            onClick={async () => {
              try {
                const code = await invoke<string>("check_llm_health");
                if (code === "healthy") {
                  setHealthWarning(null);
                } else {
                  setHealthWarning({
                    status: code,
                    message: `重試後仍異常：${code}`,
                  });
                }
              } catch (err) {
                setHealthWarning({ status: "error", message: String(err) });
              }
            }}
          >
            重試
          </button>
        </div>
      )}
      <div className="settings-layout">
        <nav className="settings-nav">
          <button
            className={activeTab === "translate" ? "active" : ""}
            onClick={() => setActiveTab("translate")}
          >
            翻譯
          </button>
          <button
            className={activeTab === "speech" ? "active" : ""}
            onClick={() => setActiveTab("speech")}
          >
            語音
          </button>
          <button
            className={activeTab === "output" ? "active" : ""}
            onClick={() => setActiveTab("output")}
          >
            輸出
          </button>
          <button
            className={activeTab === "about" ? "active" : ""}
            onClick={() => setActiveTab("about")}
          >
            關於
          </button>
        </nav>
        <main className="settings-content">
          {activeTab === "translate" && <TranslateTab />}
          {activeTab === "speech" && <SpeechTab />}
          {activeTab === "output" && <OutputTab />}
          {activeTab === "about" && <AboutTab />}
        </main>
      </div>
      <footer className="settings-footer">
        <button className="c2t-btn" onClick={hideAndReset}>
          取消
        </button>
        <button className="c2t-btn c2t-btn-primary" onClick={hideAndReset}>
          確定
        </button>
      </footer>
    </div>
  );
}
