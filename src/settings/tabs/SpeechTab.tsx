import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

type Preset = {
  id: string;
  display_name: string;
};

type WindowState = {
  speech_enabled: boolean;
  speech_active_preset: string;
};

export default function SpeechTab() {
  const [enabled, setEnabled] = useState<boolean>(true);
  const [presets, setPresets] = useState<Preset[]>([]);
  const [activePreset, setActivePreset] = useState<string>("Ryan");
  const [sampleText, setSampleText] = useState<string>("Hello, this is a voice preview.");
  const [statusMsg, setStatusMsg] = useState<string>("");

  useEffect(() => {
    void refresh();
  }, []);

  useEffect(() => {
    let disposed = false;
    let offState: null | (() => void) = null;

    const setup = async () => {
      offState = await listen<WindowState>("window-state-changed", (event) => {
        setEnabled(Boolean(event.payload.speech_enabled));
        setActivePreset(event.payload.speech_active_preset || "Ryan");
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

  async function refresh() {
    try {
      const [list, ws] = await Promise.all([
        invoke<Preset[]>("list_voice_presets"),
        invoke<WindowState>("get_window_state"),
      ]);
      setPresets(list);
      setEnabled(Boolean(ws.speech_enabled));
      setActivePreset(ws.speech_active_preset || "Ryan");
      setStatusMsg("");
    } catch (err) {
      setStatusMsg(String(err));
    }
  }

  async function updateEnabled(v: boolean) {
    setEnabled(v);
    try {
      await invoke("set_speech_enabled", { value: v });
      setStatusMsg(v ? "Speech enabled" : "Speech disabled");
    } catch (err) {
      setStatusMsg(String(err));
    }
  }

  async function selectPreset(id: string) {
    try {
      await invoke("set_active_preset", { id });
      setActivePreset(id);
      setStatusMsg(`Active preset: ${id}`);
    } catch (err) {
      setStatusMsg(String(err));
    }
  }

  async function previewPreset(id: string) {
    try {
      await invoke("preview_preset", { id, text: sampleText, lang: "zh-TW" });
      setStatusMsg(`Preview started: ${id}`);
    } catch (err) {
      setStatusMsg(String(err));
    }
  }

  return (
    <div className="settings-translate-root">
      <section className="settings-section">
        <label className="settings-checkbox">
          <input
            type="checkbox"
            checked={enabled}
            onChange={(event) => {
              void updateEnabled(event.target.checked);
            }}
          />
          Enable speech output
        </label>
      </section>

      <section className="settings-section">
        <h2>Active preset</h2>
        <div>{activePreset}</div>
      </section>

      <section className="settings-section">
        <h2>Voice presets (9)</h2>
        <ul className="preset-list">
          {presets.map((preset) => (
            <li
              key={preset.id}
              className={`preset-item ${preset.id === activePreset ? "active" : ""}`}
            >
              <div className="preset-name">{preset.display_name}</div>
              <div className="preset-actions">
                <button
                  className="c2t-btn"
                  onClick={() => {
                    void previewPreset(preset.id);
                  }}
                >
                  Preview
                </button>
                <button
                  className="c2t-btn c2t-btn-primary"
                  onClick={() => {
                    void selectPreset(preset.id);
                  }}
                  disabled={preset.id === activePreset}
                >
                  {preset.id === activePreset ? "Using" : "Set active"}
                </button>
              </div>
            </li>
          ))}
        </ul>
      </section>

      <section className="settings-section">
        <h2>Preview text</h2>
        <textarea
          value={sampleText}
          onChange={(event) => {
            setSampleText(event.target.value);
          }}
          rows={3}
          style={{ width: "100%" }}
        />
      </section>

      {statusMsg && <div className="settings-status">{statusMsg}</div>}
    </div>
  );
}
