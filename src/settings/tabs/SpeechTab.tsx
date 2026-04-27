import { useEffect, useMemo, useState } from "react";
import {
  deleteAzureCredentials,
  getAzureCredentialsStatus,
  getVoiceRouting,
  listAzureVoices,
  saveAzureCredentials,
  setVoiceRouting,
  testAzureConnection,
  type AzureCredentialsStatus,
  type AzureVoice,
} from "../../services/tts";

type LangCode = "zh-TW" | "en-US" | "de-DE" | "fr-FR" | "ja-JP" | "ko-KR";
type TestStatus = "idle" | "testing" | "ok" | "error";

const LANGUAGES: { code: LangCode; label: string; fallback: string }[] = [
  { code: "zh-TW", label: "繁體中文", fallback: "zh-TW-HsiaoChenNeural" },
  { code: "en-US", label: "English", fallback: "en-US-AvaMultilingualNeural" },
  { code: "de-DE", label: "Deutsch", fallback: "de-DE-SeraphinaMultilingualNeural" },
  { code: "fr-FR", label: "Français", fallback: "fr-FR-VivienneMultilingualNeural" },
  { code: "ja-JP", label: "日本語", fallback: "ja-JP-NanamiNeural" },
  { code: "ko-KR", label: "한국어", fallback: "ko-KR-SunHiNeural" },
];

const REGIONS = [
  { id: "eastasia", label: "East Asia" },
  { id: "southeastasia", label: "Southeast Asia" },
  { id: "japaneast", label: "Japan East" },
  { id: "koreacentral", label: "Korea Central" },
  { id: "eastus", label: "East US" },
  { id: "westeurope", label: "West Europe" },
];

const EMPTY_STATUS: AzureCredentialsStatus = {
  configured: false,
  region: null,
};

export default function SpeechTab() {
  const [keyInput, setKeyInput] = useState("");
  const [keyVisible, setKeyVisible] = useState(false);
  const [region, setRegion] = useState("eastasia");
  const [credStatus, setCredStatus] = useState<AzureCredentialsStatus>(EMPTY_STATUS);
  const [testStatus, setTestStatus] = useState<TestStatus>("idle");
  const [testError, setTestError] = useState("");
  const [statusMsg, setStatusMsg] = useState("");
  const [voicesByLang, setVoicesByLang] = useState<Record<string, AzureVoice[]>>({});
  const [routing, setRouting] = useState<Record<string, string>>({});
  const [loadingVoices, setLoadingVoices] = useState(false);
  const [saving, setSaving] = useState(false);

  const canLoadVoices = useMemo(
    () => credStatus.configured && testStatus !== "testing",
    [credStatus.configured, testStatus],
  );

  useEffect(() => {
    void refreshInitial();
  }, []);

  async function refreshInitial() {
    try {
      const [status, route] = await Promise.all([
        getAzureCredentialsStatus(),
        getVoiceRouting(),
      ]);
      setCredStatus(status);
      setRouting(route);
      if (status.region) {
        setRegion(status.region);
      }
      if (status.configured) {
        setTestStatus("idle");
        await loadVoices();
      }
    } catch (err) {
      setStatusMsg(String(err));
    }
  }

  async function saveAndTest() {
    const key = keyInput.trim();
    if (!key) {
      setStatusMsg("請輸入 Azure subscription key。");
      return;
    }
    try {
      setSaving(true);
      setStatusMsg("");
      setTestError("");
      await saveAzureCredentials(key, region);
      setKeyInput("");
      const status = await getAzureCredentialsStatus();
      setCredStatus(status);
      await testAndLoadVoices();
    } catch (err) {
      setTestStatus("error");
      setTestError(String(err));
    } finally {
      setSaving(false);
    }
  }

  async function deleteCredentials() {
    try {
      setSaving(true);
      await deleteAzureCredentials();
      setCredStatus(EMPTY_STATUS);
      setKeyInput("");
      setVoicesByLang({});
      setRouting({});
      setTestStatus("idle");
      setTestError("");
      setStatusMsg("Azure key 已移除。");
    } catch (err) {
      setStatusMsg(String(err));
    } finally {
      setSaving(false);
    }
  }

  async function testAndLoadVoices() {
    try {
      setTestStatus("testing");
      setTestError("");
      setStatusMsg("");
      await testAzureConnection();
      setTestStatus("ok");
      setStatusMsg("Azure 連線成功。");
      await loadVoices();
    } catch (err) {
      setTestStatus("error");
      setTestError(String(err));
    }
  }

  async function loadVoices() {
    try {
      setLoadingVoices(true);
      const entries = await Promise.all(
        LANGUAGES.map(async (item) => [item.code, await listAzureVoices(item.code)] as const),
      );
      setVoicesByLang(Object.fromEntries(entries));
    } catch (err) {
      setTestStatus("error");
      setTestError(String(err));
    } finally {
      setLoadingVoices(false);
    }
  }

  async function changeVoice(lang: LangCode, voiceId: string) {
    try {
      await setVoiceRouting(lang, voiceId);
      setRouting((prev) => ({ ...prev, [lang]: voiceId }));
      setStatusMsg(`${lang} voice 已更新。`);
    } catch (err) {
      setStatusMsg(String(err));
    }
  }

  return (
    <div className="settings-translate-root">
      <section className="settings-section">
        <h2>Azure TTS</h2>
        <div className="settings-editor">
          <label>
            Subscription key
            <div style={{ display: "flex", gap: 8 }}>
              <input
                type={keyVisible ? "text" : "password"}
                value={keyInput}
                placeholder={credStatus.configured ? "已設定，重新輸入可覆蓋" : "輸入 Azure key"}
                onChange={(event) => setKeyInput(event.target.value)}
              />
              <button className="c2t-btn" type="button" onClick={() => setKeyVisible((v) => !v)}>
                {keyVisible ? "隱藏" : "顯示"}
              </button>
            </div>
          </label>

          <label>
            Region
            <select value={region} onChange={(event) => setRegion(event.target.value)}>
              {REGIONS.map((item) => (
                <option key={item.id} value={item.id}>
                  {item.label} ({item.id})
                </option>
              ))}
            </select>
          </label>

          <div className="settings-editor-actions">
            <button
              className="c2t-btn c2t-btn-primary"
              type="button"
              disabled={saving}
              onClick={() => void saveAndTest()}
            >
              儲存並測試
            </button>
            <button
              className="c2t-btn"
              type="button"
              disabled={!credStatus.configured || testStatus === "testing"}
              onClick={() => void testAndLoadVoices()}
            >
              測試連線
            </button>
            <button
              className="c2t-btn"
              type="button"
              disabled={!credStatus.configured || saving}
              onClick={() => void deleteCredentials()}
            >
              移除 key
            </button>
          </div>

          <div className="settings-output-log-hint">
            狀態：
            {credStatus.configured ? `已設定 (${credStatus.region ?? region})` : "尚未設定"}
            {testStatus === "testing" && "，測試中..."}
            {testStatus === "ok" && "，連線成功"}
            {testStatus === "error" && "，連線失敗"}
          </div>
          {testError && <div className="settings-status">{testError}</div>}
        </div>
      </section>

      <section className="settings-section">
        <div className="settings-voice-header">
          <h2>各語言 voice</h2>
          <button
            className="c2t-btn"
            type="button"
            disabled={!canLoadVoices || loadingVoices}
            onClick={() => void loadVoices()}
          >
            {loadingVoices ? "載入中" : "重新載入 voices"}
          </button>
        </div>

        <div className="settings-editor">
          {LANGUAGES.map((item) => {
            const voices = voicesByLang[item.code] ?? [];
            const selected = routing[item.code] || item.fallback;
            return (
              <label key={item.code}>
                {item.label} ({item.code})
                <select
                  value={selected}
                  disabled={!credStatus.configured || voices.length === 0}
                  onChange={(event) => void changeVoice(item.code, event.target.value)}
                >
                  {voices.length === 0 && <option value={item.fallback}>{item.fallback}</option>}
                  {voices.map((voice) => (
                    <option key={voice.id} value={voice.id}>
                      {voiceLabel(voice)}
                    </option>
                  ))}
                </select>
              </label>
            );
          })}
        </div>
      </section>

      {statusMsg && <div className="settings-status">{statusMsg}</div>}
    </div>
  );
}

function voiceLabel(voice: AzureVoice): string {
  const tags: string[] = [];
  if (voice.id.toLowerCase().includes("multilingual")) {
    tags.push("Multilingual");
  }
  if (voice.level === "HighDefinition") {
    tags.push("HD");
  }
  const suffix = tags.length > 0 ? ` (${tags.join(", ")})` : "";
  return `${voice.name}${suffix} - ${voice.id}`;
}
