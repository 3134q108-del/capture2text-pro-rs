import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import {
  deleteAzureCredentials,
  getAzureCredentialsStatus,
  getAzureUsageInfo,
  getSpeechRate,
  getVoiceRouting,
  listAzureVoices,
  previewVoice,
  saveAzureCredentials,
  setBillingTier,
  setHdLimit,
  setNeuralLimit,
  setSpeechRate,
  setVoiceRouting,
  testAzureConnection,
  type AzureCredentialsStatus,
  type AzureVoice,
  type BillingTier,
  type UsageInfo,
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

const DEFAULT_USAGE: UsageInfo = {
  tier: "F0",
  neural_used: 0,
  hd_used: 0,
  neural_limit: 500_000,
  hd_limit: 0,
  month: "",
  neural_percent: 0,
  hd_percent: 0,
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
  const [speechRate, setSpeechRateState] = useState(1.0);
  const [rateLoaded, setRateLoaded] = useState(false);
  const [previewingLang, setPreviewingLang] = useState<string | null>(null);
  const [usage, setUsage] = useState<UsageInfo>(DEFAULT_USAGE);
  const [tier, setTier] = useState<BillingTier>("F0");
  const [neuralLimitDraft, setNeuralLimitDraft] = useState("1000000");
  const [hdLimitDraft, setHdLimitDraft] = useState("100000");
  const [limitsLoaded, setLimitsLoaded] = useState(false);
  const previewTimerRef = useRef<number | null>(null);

  const canLoadVoices = useMemo(
    () => credStatus.configured && testStatus !== "testing",
    [credStatus.configured, testStatus],
  );

  useEffect(() => {
    void refreshInitial();
    const usageTimer = window.setInterval(() => {
      void refreshUsage();
    }, 30_000);
    return () => {
      window.clearInterval(usageTimer);
      if (previewTimerRef.current !== null) {
        window.clearTimeout(previewTimerRef.current);
      }
    };
  }, []);

  useEffect(() => {
    if (!rateLoaded) {
      return;
    }
    const timer = window.setTimeout(() => {
      void setSpeechRate(speechRate);
    }, 250);
    return () => window.clearTimeout(timer);
  }, [rateLoaded, speechRate]);

  useEffect(() => {
    if (!limitsLoaded || tier !== "S0") {
      return;
    }
    const timer = window.setTimeout(() => {
      const limit = parseLimit(neuralLimitDraft);
      if (limit !== null) {
        void setNeuralLimit(limit).then(refreshUsage);
      }
    }, 250);
    return () => window.clearTimeout(timer);
  }, [limitsLoaded, neuralLimitDraft, tier]);

  useEffect(() => {
    if (!limitsLoaded || tier !== "S0") {
      return;
    }
    const timer = window.setTimeout(() => {
      const limit = parseLimit(hdLimitDraft);
      if (limit !== null) {
        void setHdLimit(limit).then(refreshUsage);
      }
    }, 250);
    return () => window.clearTimeout(timer);
  }, [limitsLoaded, hdLimitDraft, tier]);

  async function refreshInitial() {
    try {
      const [status, route, rate, usageInfo] = await Promise.all([
        getAzureCredentialsStatus(),
        getVoiceRouting(),
        getSpeechRate(),
        getAzureUsageInfo(),
      ]);
      setCredStatus(status);
      setRouting(route);
      setSpeechRateState(rate);
      setRateLoaded(true);
      applyUsage(usageInfo);
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

  async function refreshUsage() {
    try {
      applyUsage(await getAzureUsageInfo());
    } catch (err) {
      setStatusMsg(String(err));
    }
  }

  function applyUsage(info: UsageInfo) {
    setUsage(info);
    setTier(info.tier);
    setNeuralLimitDraft(String(info.neural_limit || 1));
    setHdLimitDraft(String(info.hd_limit || 1));
    setLimitsLoaded(true);
  }

  async function saveAndTest() {
    const key = keyInput.trim();
    if (!key) {
      setStatusMsg("請輸入 Azure 訂閱金鑰。");
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
      setStatusMsg("Azure 金鑰已移除。");
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
      setStatusMsg("Azure 連線正常。");
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

  async function switchTier(nextTier: BillingTier) {
    try {
      setTier(nextTier);
      await setBillingTier(nextTier);
      await refreshUsage();
    } catch (err) {
      setStatusMsg(String(err));
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

  async function handlePreview(lang: LangCode) {
    const item = LANGUAGES.find((entry) => entry.code === lang);
    const voiceId = item ? selectedVoiceFor(item) : undefined;
    if (!voiceId) {
      return;
    }
    if (previewTimerRef.current !== null) {
      window.clearTimeout(previewTimerRef.current);
      previewTimerRef.current = null;
    }
    setPreviewingLang(lang);
    setStatusMsg("");
    try {
      await previewVoice(lang, voiceId);
      await refreshUsage();
    } catch (err) {
      setStatusMsg(`試聽失敗：${String(err)}`);
    } finally {
      previewTimerRef.current = window.setTimeout(() => {
        setPreviewingLang(null);
        previewTimerRef.current = null;
      }, 8000);
    }
  }

  function selectedVoiceFor(item: { code: LangCode; fallback: string }): string {
    const routed = routing[item.code] || item.fallback;
    if (isVoiceUsable(routed, tier)) {
      return routed;
    }
    const usableVoice = (voicesByLang[item.code] ?? []).find((voice) =>
      isVoiceUsable(voice.id, tier),
    );
    return usableVoice?.id ?? item.fallback;
  }

  return (
    <div className="settings-translate-root">
      <section className="settings-section">
        <h2>Azure TTS</h2>
        <div className="settings-editor">
          <label>
            訂閱金鑰
            <div className="settings-key-row">
              <input
                type={keyVisible ? "text" : "password"}
                value={keyInput}
                placeholder={credStatus.configured ? "已設定，重新輸入可更新金鑰" : "輸入 Azure 金鑰"}
                onChange={(event) => setKeyInput(event.target.value)}
              />
              <button className="c2t-btn" type="button" onClick={() => setKeyVisible((v) => !v)}>
                {keyVisible ? "隱藏" : "顯示"}
              </button>
              <button
                className="c2t-btn"
                type="button"
                disabled={!credStatus.configured || saving}
                onClick={() => void deleteCredentials()}
              >
                移除金鑰
              </button>
            </div>
          </label>

          <label>
            區域
            <select value={region} onChange={(event) => setRegion(event.target.value)}>
              {REGIONS.map((item) => (
                <option key={item.id} value={item.id}>
                  {item.label} ({item.id})
                </option>
              ))}
            </select>
          </label>

          <div className="billing-tier-row">
            <span>方案</span>
            <label>
              <input
                type="radio"
                checked={tier === "F0"}
                onChange={() => void switchTier("F0")}
              />
              F0 (Free)
            </label>
            <label>
              <input
                type="radio"
                checked={tier === "S0"}
                onChange={() => void switchTier("S0")}
              />
              S0 (付費)
            </label>
          </div>

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
          </div>

          <div className="settings-output-log-hint">
            狀態：
            {credStatus.configured ? `已設定 (${credStatus.region ?? region})` : "尚未設定"}
            {testStatus === "testing" && "，測試中..."}
            {testStatus === "ok" && "，連線正常"}
            {testStatus === "error" && "，連線失敗"}
          </div>
          {testError && <div className="settings-status">{testError}</div>}
        </div>
      </section>

      <section className="settings-section">
        <h2>Azure 用量</h2>
        <div className="usage-card">
          <UsageMeter
            label="Neural 語音"
            used={usage.neural_used}
            limit={usage.neural_limit}
            percent={usage.neural_percent}
            month={usage.month}
            limitInput={
              tier === "S0" ? (
                <input
                  className="usage-limit-input"
                  type="number"
                  min={1}
                  step={10000}
                  value={neuralLimitDraft}
                  onChange={(event) => setNeuralLimitDraft(event.target.value)}
                  onBlur={() => setNeuralLimitDraft(String(parseLimit(neuralLimitDraft) ?? 1))}
                />
              ) : null
            }
          />
          {tier === "S0" && (
            <UsageMeter
              label="HD 語音"
              used={usage.hd_used}
              limit={usage.hd_limit}
              percent={usage.hd_percent}
              month={usage.month}
              limitInput={
                <input
                  className="usage-limit-input"
                  type="number"
                  min={1}
                  step={10000}
                  value={hdLimitDraft}
                  onChange={(event) => setHdLimitDraft(event.target.value)}
                  onBlur={() => setHdLimitDraft(String(parseLimit(hdLimitDraft) ?? 1))}
                />
              }
            />
          )}
          <a
            className="usage-portal-link"
            href="https://portal.azure.com/#view/Microsoft_Azure_ProjectOxford/CognitiveServicesHub/~/SpeechServices"
            target="_blank"
            rel="noreferrer"
          >
            <svg className="result-usage-donut" viewBox="0 0 20 20" aria-hidden="true">
              <circle className="usage-track" cx="10" cy="10" r="7" />
              <circle
                className="usage-fill green"
                cx="10"
                cy="10"
                r="7"
                strokeDasharray="28 44"
              />
            </svg>
            Azure Portal
          </a>
        </div>
      </section>

      <section className="settings-section">
        <div className="settings-voice-header">
          <h2>朗讀 voice</h2>
          <button
            className="c2t-btn"
            type="button"
            disabled={!canLoadVoices || loadingVoices}
            onClick={() => void loadVoices()}
          >
            {loadingVoices ? "載入中" : "重新載入語音"}
          </button>
        </div>

        <div className="settings-editor">
          <div className="speech-rate-row">
            <label htmlFor="speech-rate-slider">朗讀速度</label>
            <input
              id="speech-rate-slider"
              type="range"
              min={0.5}
              max={2.0}
              step={0.05}
              value={speechRate}
              onChange={(event) => setSpeechRateState(Number(event.target.value))}
            />
            <span>{speechRate.toFixed(2)}x</span>
          </div>

          {LANGUAGES.map((item) => {
            const voices = voicesByLang[item.code] ?? [];
            const usableVoices = voices.filter((voice) => isVoiceUsable(voice.id, tier));
            const selected = selectedVoiceFor(item);
            return (
              <label key={item.code}>
                <div className="voice-row-header">
                  <span>
                    {item.label} ({item.code})
                  </span>
                  <button
                    className="c2t-btn"
                    type="button"
                    disabled={
                      !credStatus.configured || previewingLang === item.code || loadingVoices
                    }
                    onClick={() => void handlePreview(item.code)}
                  >
                    {previewingLang === item.code ? "試聽中" : "試聽"}
                  </button>
                </div>
                <select
                  value={selected}
                  disabled={!credStatus.configured || usableVoices.length === 0}
                  onChange={(event) => void changeVoice(item.code, event.target.value)}
                >
                  {usableVoices.length === 0 && <option value={item.fallback}>{item.fallback}</option>}
                  {usableVoices.map((voice) => (
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

function UsageMeter({
  label,
  used,
  limit,
  percent,
  month,
  limitInput,
}: {
  label: string;
  used: number;
  limit: number;
  percent: number;
  month: string;
  limitInput: ReactNode;
}) {
  const clampedPercent = Math.min(100, Math.max(0, percent));
  const tone = percent >= 90 ? "red" : percent >= 70 ? "yellow" : "green";
  return (
    <div className="usage-meter">
      <div className="usage-meter-header">
        <span>{label}</span>
        <span>{month}</span>
      </div>
      <div className="usage-bar-container" aria-hidden="true">
        <div className={`usage-bar ${tone}`} style={{ width: `${clampedPercent}%` }} />
      </div>
      <div className="usage-meter-footer">
        <span>
          {formatNumber(used)} / {limitInput ?? formatNumber(limit)}
        </span>
        <span>{percent.toFixed(1)}%</span>
      </div>
    </div>
  );
}

function isVoiceUsable(voiceId: string, tier: BillingTier): boolean {
  return tier === "S0" || !/HD|DragonHD/i.test(voiceId);
}

function parseLimit(value: string): number | null {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed)) {
    return null;
  }
  return Math.max(1, parsed);
}

function formatNumber(value: number): string {
  return new Intl.NumberFormat("en-US").format(value);
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
