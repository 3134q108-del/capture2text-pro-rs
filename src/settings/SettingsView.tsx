import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./SettingsView.css";

type Scenario = {
  id: string;
  name: string;
  prompt: string;
  builtin: boolean;
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
  const [statusMsg, setStatusMsg] = useState<string>("");

  useEffect(() => {
    void refresh();
  }, []);

  const selectedScenario = useMemo(
    () => scenarios.find((item) => item.id === selectedId) ?? null,
    [scenarios, selectedId],
  );

  async function refresh() {
    try {
      const list = await invoke<Scenario[]>("list_scenarios");
      const active = await invoke<string>("get_active_scenario");
      setScenarios(list);
      setActiveId(active);
      const fallback = list.find((item) => item.id === active) ?? list[0] ?? null;
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
      name: "新情境",
      prompt: "",
      builtin: false,
    };
    setSelectedId(id);
    setDraft(next);
    setStatusMsg("新情境尚未儲存");
  }

  async function saveScenario() {
    const id = draft.id.trim();
    const name = draft.name.trim();
    if (!id) {
      setStatusMsg("ID 不能為空");
      return;
    }
    if (!name) {
      setStatusMsg("名稱不能為空");
      return;
    }
    try {
      await invoke("save_scenario", {
        scenario: { ...draft, id, name, prompt: draft.prompt },
      });
      await refresh();
      setSelectedId(id);
      setStatusMsg("已儲存");
    } catch (err) {
      setStatusMsg(String(err));
    }
  }

  async function deleteScenario() {
    if (!selectedScenario) return;
    try {
      await invoke("delete_scenario", { id: selectedScenario.id });
      await refresh();
      setStatusMsg("已刪除");
    } catch (err) {
      setStatusMsg(String(err));
    }
  }

  async function applyActiveScenario() {
    if (!draft.id.trim()) return;
    try {
      await invoke("set_active_scenario", { id: draft.id.trim() });
      await refresh();
      setStatusMsg("已設為使用中");
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
        <h1>情境管理</h1>
        <button className="settings-btn" onClick={closeSettings}>
          關閉
        </button>
      </header>
      <div className="settings-layout">
        <aside className="settings-sidebar">
          <div className="settings-sidebar-actions">
            <button className="settings-btn" onClick={createScenario}>
              新增
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
                    {item.id === activeId && <small className="badge active">使用中</small>}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        </aside>
        <main className="settings-editor">
          <label>
            ID
            <input
              value={draft.id}
              disabled={draft.builtin}
              onChange={(event) => setDraft((prev) => ({ ...prev, id: event.target.value }))}
            />
          </label>
          <label>
            名稱
            <input
              value={draft.name}
              onChange={(event) => setDraft((prev) => ({ ...prev, name: event.target.value }))}
            />
          </label>
          <label className="settings-prompt-label">
            Prompt
            <textarea
              value={draft.prompt}
              onChange={(event) => setDraft((prev) => ({ ...prev, prompt: event.target.value }))}
            />
          </label>
          <div className="settings-editor-actions">
            <button className="settings-btn primary" onClick={saveScenario}>
              儲存
            </button>
            <button className="settings-btn" onClick={applyActiveScenario}>
              設為使用中
            </button>
            <button
              className="settings-btn danger"
              onClick={deleteScenario}
              disabled={Boolean(selectedScenario?.builtin)}
            >
              刪除
            </button>
          </div>
          {statusMsg && <div className="settings-status">{statusMsg}</div>}
        </main>
      </div>
    </div>
  );
}
