import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useMemo, useState } from "react";

type Scenario = {
  id: string;
  name: string;
  prompt: string;
  builtin: boolean;
};

type OutputLang = "zh-TW" | "zh-CN" | "en-US" | "ja-JP" | "ko-KR" | "de-DE" | "fr-FR";

const EMPTY_SCENARIO: Scenario = {
  id: "",
  name: "",
  prompt: "",
  builtin: false,
};

const LANG_OPTIONS: { code: OutputLang; label: string }[] = [
  { code: "zh-TW", label: "繁體中文" },
  { code: "zh-CN", label: "简体中文" },
  { code: "en-US", label: "English" },
  { code: "ja-JP", label: "日本語" },
  { code: "ko-KR", label: "한국어" },
  { code: "de-DE", label: "Deutsch" },
  { code: "fr-FR", label: "Français" },
];

function normalizeLang(s: string): OutputLang {
  return (
    ["zh-TW", "zh-CN", "en-US", "ja-JP", "ko-KR", "de-DE", "fr-FR"] as const
  ).includes(s as OutputLang)
    ? (s as OutputLang)
    : "zh-TW";
}

export default function TranslateTab() {
  const [scenarios, setScenarios] = useState<Scenario[]>([]);
  const [activeId, setActiveId] = useState<string>("default");
  const [selectedId, setSelectedId] = useState<string>("");
  const [draft, setDraft] = useState<Scenario>(EMPTY_SCENARIO);
  const [outputLang, setOutputLang] = useState<OutputLang>("zh-TW");
  const [statusMsg, setStatusMsg] = useState<string>("");

  const selectedScenario = useMemo(
    () => scenarios.find((item) => item.id === selectedId) ?? null,
    [scenarios, selectedId],
  );

  useEffect(() => {
    void refresh();
  }, []);

  useEffect(() => {
    let disposed = false;
    let offLang: null | (() => void) = null;

    const setup = async () => {
      offLang = await listen<string>("output-language-changed", (event) => {
        setOutputLang(normalizeLang(event.payload));
      });
      if (disposed) {
        offLang?.();
        offLang = null;
      }
    };

    void setup();
    return () => {
      disposed = true;
      offLang?.();
    };
  }, []);

  async function refresh() {
    try {
      const [list, active, lang] = await Promise.all([
        invoke<Scenario[]>("list_scenarios"),
        invoke<string>("get_active_scenario"),
        invoke<string>("get_output_language"),
      ]);

      setScenarios(list);
      setActiveId(active);
      setOutputLang(normalizeLang(lang));

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

  async function changeOutputLang(next: OutputLang) {
    try {
      await invoke("set_output_language", { lang: next });
      setOutputLang(next);
      setStatusMsg("輸出語言已更新");
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
    setStatusMsg("已建立新情境草稿");
  }

  async function saveScenario() {
    const id = draft.id.trim();
    const name = draft.name.trim();
    if (!id) {
      setStatusMsg("情境 ID 不可空白");
      return;
    }
    if (!name) {
      setStatusMsg("情境名稱不可空白");
      return;
    }
    try {
      await invoke("save_scenario", {
        scenario: { ...draft, id, name, prompt: draft.prompt },
      });
      await refresh();
      setSelectedId(id);
      setStatusMsg("情境已儲存");
    } catch (err) {
      setStatusMsg(String(err));
    }
  }

  async function deleteScenario() {
    if (!selectedScenario) return;
    try {
      await invoke("delete_scenario", { id: selectedScenario.id });
      await refresh();
      setStatusMsg("情境已刪除");
    } catch (err) {
      setStatusMsg(String(err));
    }
  }

  async function applyActiveScenario() {
    const id = draft.id.trim();
    if (!id) return;
    try {
      await invoke("set_active_scenario", { id });
      await refresh();
      setStatusMsg("已套用為預設情境");
    } catch (err) {
      setStatusMsg(String(err));
    }
  }

  return (
    <div className="settings-translate-root">
      <section className="settings-section">
        <h2>輸出語言</h2>
        <div className="settings-radio-row">
          {LANG_OPTIONS.map((opt) => (
            <label key={opt.code}>
              <input
                type="radio"
                name="output-lang"
                checked={outputLang === opt.code}
                onChange={() => changeOutputLang(opt.code)}
              />
              {opt.label}
            </label>
          ))}
        </div>
      </section>

      <section className="settings-section">
        <h2>翻譯情境</h2>
        <div className="settings-tab-layout">
          <aside className="settings-sidebar">
            <div className="settings-sidebar-actions">
              <button className="c2t-btn" onClick={createScenario}>
                新增情境
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
                      {item.builtin && <small className="badge">內建</small>}
                      {item.id === activeId && <small className="badge active">使用中</small>}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          </aside>
          <main className="settings-editor">
            <label>
              情境 ID
              <input
                value={draft.id}
                disabled={draft.builtin}
                onChange={(event) => setDraft((prev) => ({ ...prev, id: event.target.value }))}
              />
            </label>
            <label>
              情境名稱
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
              <button className="c2t-btn c2t-btn-primary" onClick={saveScenario}>
                儲存
              </button>
              <button className="c2t-btn" onClick={applyActiveScenario}>
                套用為使用中
              </button>
              <button className="c2t-btn" onClick={deleteScenario} disabled={Boolean(selectedScenario?.builtin)}>
                刪除
              </button>
            </div>
            <div className="settings-output-log-hint">
              新增情境後可在 Result 視窗重翻譯時使用；內建情境不可刪除。
            </div>
            {statusMsg && <div className="settings-status">{statusMsg}</div>}
          </main>
        </div>
      </section>
    </div>
  );
}
