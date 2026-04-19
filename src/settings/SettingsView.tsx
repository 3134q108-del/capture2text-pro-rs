import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./SettingsView.css";

type Scenario = {
  id: string;
  name: string;
  prompt: string;
  builtin: boolean;
};

type TtsVoiceOption = {
  code: string;
  display_name: string;
  lang: string;
};

type TtsConfig = {
  active_zh: string;
  active_en: string;
};

const EMPTY_SCENARIO: Scenario = {
  id: "",
  name: "",
  prompt: "",
  builtin: false,
};

export default function SettingsView() {
  const [scenarios, setScenarios] = useState<Scenario[]>([]);
  const [activeId, setActiveId] = useState<string>("default");
  const [selectedId, setSelectedId] = useState<string>("");
  const [draft, setDraft] = useState<Scenario>(EMPTY_SCENARIO);
  const [voices, setVoices] = useState<TtsVoiceOption[]>([]);
  const [ttsConfig, setTtsConfig] = useState<TtsConfig>({
    active_zh: "",
    active_en: "",
  });
  const [statusMsg, setStatusMsg] = useState<string>("");

  useEffect(() => {
    void refresh();
  }, []);

  const selectedScenario = useMemo(
    () => scenarios.find((item) => item.id === selectedId) ?? null,
    [scenarios, selectedId],
  );

  const zhVoices = useMemo(
    () => voices.filter((voice) => voice.lang === "zh"),
    [voices],
  );
  const enVoices = useMemo(
    () => voices.filter((voice) => voice.lang === "en"),
    [voices],
  );

  async function refresh() {
    try {
      const [list, active, voiceList, config] = await Promise.all([
        invoke<Scenario[]>("list_scenarios"),
        invoke<string>("get_active_scenario"),
        invoke<TtsVoiceOption[]>("list_tts_voices"),
        invoke<TtsConfig>("get_tts_config"),
      ]);

      setScenarios(list);
      setActiveId(active);
      setVoices(voiceList);
      setTtsConfig(config);

      const fallback =
        list.find((item) => item.id === selectedId) ??
        list.find((item) => item.id === active) ??
        list[0] ??
        null;
      if (fallback) {
        setSelectedId(fallback.id);
        setDraft({ ...fallback });
      }
      setStatusMsg("");
    } catch (err) {
      setStatusMsg(String(err));
    }
  }

  function selectScenario(id: string) {
    const item = scenarios.find((x) => x.id === id);
    if (!item) return;
    setSelectedId(id);
    setDraft({ ...item });
    setStatusMsg("");
  }

  function createScenario() {
    const id = `custom_${Date.now()}`;
    const next: Scenario = {
      id,
      name: "New Scenario",
      prompt: "",
      builtin: false,
    };
    setSelectedId(id);
    setDraft(next);
    setStatusMsg("Created draft scenario.");
  }

  async function saveScenario() {
    const id = draft.id.trim();
    const name = draft.name.trim();
    if (!id) {
      setStatusMsg("Scenario ID is required.");
      return;
    }
    if (!name) {
      setStatusMsg("Scenario name is required.");
      return;
    }
    try {
      await invoke("save_scenario", {
        scenario: { ...draft, id, name, prompt: draft.prompt },
      });
      await refresh();
      setSelectedId(id);
      setStatusMsg("Scenario saved.");
    } catch (err) {
      setStatusMsg(String(err));
    }
  }

  async function deleteScenario() {
    if (!selectedScenario) return;
    try {
      await invoke("delete_scenario", { id: selectedScenario.id });
      await refresh();
      setStatusMsg("Scenario deleted.");
    } catch (err) {
      setStatusMsg(String(err));
    }
  }

  async function applyActiveScenario() {
    if (!draft.id.trim()) return;
    try {
      await invoke("set_active_scenario", { id: draft.id.trim() });
      await refresh();
      setStatusMsg("Active scenario updated.");
    } catch (err) {
      setStatusMsg(String(err));
    }
  }

  async function setVoice(lang: "zh" | "en", code: string) {
    try {
      await invoke("set_tts_voice", { lang, code });
      setTtsConfig((prev) =>
        lang === "zh"
          ? { ...prev, active_zh: code }
          : { ...prev, active_en: code },
      );
      setStatusMsg("TTS voice updated.");
    } catch (err) {
      setStatusMsg(String(err));
    }
  }

  async function closeSettings() {
    try {
      await invoke("hide_settings_window");
    } catch {
      // ignore
    }
  }

  return (
    <div className="settings-root">
      <header className="settings-header">
        <h1>Settings</h1>
        <button className="settings-btn" onClick={closeSettings}>
          Close
        </button>
      </header>
      <div className="settings-layout">
        <aside className="settings-sidebar">
          <div className="settings-sidebar-actions">
            <button className="settings-btn" onClick={createScenario}>
              New Scenario
            </button>
          </div>
          <ul className="settings-list">
            {scenarios.map((item) => (
              <li key={item.id}>
                <button
                  className={`settings-list-item ${item.id === selectedId ? "active" : ""}`}
                  onClick={() => selectScenario(item.id)}
                >
                  <span>{item.name}</span>
                  <span className="settings-badges">
                    {item.builtin && <small className="badge">Built-in</small>}
                    {item.id === activeId && (
                      <small className="badge active">Active</small>
                    )}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        </aside>
        <main className="settings-editor">
          <label>
            Scenario ID
            <input
              value={draft.id}
              disabled={draft.builtin}
              onChange={(event) =>
                setDraft((prev) => ({ ...prev, id: event.target.value }))
              }
            />
          </label>
          <label>
            Scenario Name
            <input
              value={draft.name}
              onChange={(event) =>
                setDraft((prev) => ({ ...prev, name: event.target.value }))
              }
            />
          </label>
          <label className="settings-prompt-label">
            Prompt
            <textarea
              value={draft.prompt}
              onChange={(event) =>
                setDraft((prev) => ({ ...prev, prompt: event.target.value }))
              }
            />
          </label>
          <div className="settings-editor-actions">
            <button className="settings-btn primary" onClick={saveScenario}>
              Save
            </button>
            <button className="settings-btn" onClick={applyActiveScenario}>
              Set Active
            </button>
            <button
              className="settings-btn danger"
              onClick={deleteScenario}
              disabled={Boolean(selectedScenario?.builtin)}
            >
              Delete
            </button>
          </div>

          <section className="settings-tts">
            <h2>TTS Voice</h2>
            <div className="settings-tts-grid">
              <label>
                Chinese (zh-TW)
                <select
                  value={ttsConfig.active_zh}
                  onChange={(event) => setVoice("zh", event.target.value)}
                >
                  {zhVoices.map((voice) => (
                    <option key={voice.code} value={voice.code}>
                      {voice.display_name}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                English (en-US)
                <select
                  value={ttsConfig.active_en}
                  onChange={(event) => setVoice("en", event.target.value)}
                >
                  {enVoices.map((voice) => (
                    <option key={voice.code} value={voice.code}>
                      {voice.display_name}
                    </option>
                  ))}
                </select>
              </label>
            </div>
          </section>

          {statusMsg && <div className="settings-status">{statusMsg}</div>}
        </main>
      </div>
    </div>
  );
}
