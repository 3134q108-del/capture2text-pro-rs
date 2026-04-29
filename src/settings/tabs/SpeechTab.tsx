import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import {
  deleteAzureCredentials,
  getAzureCredentialsStatus,
  getAzureUsageInfo,
  getSpeechRate,
  getSpeechVolume,
  getVoiceRouting,
  listAzureVoices,
  previewVoice,
  saveAzureCredentials,
  setBillingTier,
  setHdLimit,
  setNeuralLimit,
  setSpeechRate,
  setSpeechVolume,
  setVoiceRouting,
  testAzureConnection,
  type AzureCredentialsStatus,
  type AzureVoice,
  type BillingTier,
  type UsageInfo,
} from "../../services/tts";
import {
  Button,
  FormField,
  Input,
  ProgressBar,
  RadioGroup,
  RadioGroupItem,
  Section,
  SectionBody,
  SectionHeader,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  Slider,
  StatusText,
  UsageDonut,
} from "@/components/ui";

type LangCode = "zh-TW" | "en-US" | "de-DE" | "fr-FR" | "ja-JP" | "ko-KR";
type TestStatus = "idle" | "testing" | "ok" | "error";

const LANGUAGES: Array<{ code: LangCode; label: string; fallback: string; sample: string }> = [
  { code: "zh-TW", label: "繁體中文", fallback: "zh-TW-HsiaoChenNeural", sample: "春暖花開，歡迎試聽。" },
  { code: "en-US", label: "English", fallback: "en-US-AvaMultilingualNeural", sample: "The quick brown fox jumps over the lazy dog." },
  { code: "de-DE", label: "Deutsch", fallback: "de-DE-SeraphinaMultilingualNeural", sample: "Franz jagt im komplett verwahrlosten Taxi quer durch Bayern." },
  { code: "fr-FR", label: "Français", fallback: "fr-FR-VivienneMultilingualNeural", sample: "Portez ce vieux whisky au juge blond qui fume." },
  { code: "ja-JP", label: "日本語", fallback: "ja-JP-NanamiNeural", sample: "いろはにほへと、ちりぬるを。" },
  { code: "ko-KR", label: "한국어", fallback: "ko-KR-SunHiNeural", sample: "빠른 갈색 여우가 게으른 개를 뛰어넘습니다." },
];

const REGIONS: Array<{ id: string; label: string }> = [
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
  const [speechVolume, setSpeechVolumeState] = useState(1.0);
  const [rateLoaded, setRateLoaded] = useState(false);
  const [volumeLoaded, setVolumeLoaded] = useState(false);
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
    return () => {
      window.clearTimeout(timer);
    };
  }, [rateLoaded, speechRate]);

  useEffect(() => {
    if (!volumeLoaded) {
      return;
    }
    const timer = window.setTimeout(() => {
      void setSpeechVolume(speechVolume);
    }, 250);
    return () => {
      window.clearTimeout(timer);
    };
  }, [volumeLoaded, speechVolume]);

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
    return () => {
      window.clearTimeout(timer);
    };
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
    return () => {
      window.clearTimeout(timer);
    };
  }, [limitsLoaded, hdLimitDraft, tier]);

  async function refreshInitial() {
    try {
      const [status, route, rate, volume, usageInfo] = await Promise.all([
        getAzureCredentialsStatus(),
        getVoiceRouting(),
        getSpeechRate(),
        getSpeechVolume(),
        getAzureUsageInfo(),
      ]);
      setCredStatus(status);
      setRouting(route);
      setSpeechRateState(rate);
      setSpeechVolumeState(volume);
      setRateLoaded(true);
      setVolumeLoaded(true);
      applyUsage(usageInfo);

      if (status.region) {
        setRegion(status.region);
      }
      if (status.configured) {
        setTestStatus("idle");
        await loadVoices();
      }
    } catch (error) {
      setStatusMsg(String(error));
    }
  }

  async function refreshUsage() {
    try {
      applyUsage(await getAzureUsageInfo());
    } catch (error) {
      setStatusMsg(String(error));
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
    } catch (error) {
      setTestStatus("error");
      setTestError(String(error));
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
    } catch (error) {
      setStatusMsg(String(error));
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
    } catch (error) {
      setTestStatus("error");
      setTestError(String(error));
    }
  }

  async function loadVoices() {
    try {
      setLoadingVoices(true);
      const entries = await Promise.all(
        LANGUAGES.map(async (item) => [item.code, await listAzureVoices(item.code)] as const),
      );
      setVoicesByLang(Object.fromEntries(entries));
    } catch (error) {
      setTestStatus("error");
      setTestError(String(error));
    } finally {
      setLoadingVoices(false);
    }
  }

  async function switchTier(nextTier: BillingTier) {
    try {
      setTier(nextTier);
      await setBillingTier(nextTier);
      await refreshUsage();
    } catch (error) {
      setStatusMsg(String(error));
    }
  }

  async function changeVoice(lang: LangCode, voiceId: string) {
    try {
      await setVoiceRouting(lang, voiceId);
      setRouting((prev) => ({ ...prev, [lang]: voiceId }));
      setStatusMsg(`${lang} voice 已更新。`);
    } catch (error) {
      setStatusMsg(String(error));
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
    } catch (error) {
      setStatusMsg(`試聽失敗: ${String(error)}`);
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

  const usagePercent = tier === "S0"
    ? Math.max(usage.neural_percent, usage.hd_percent)
    : usage.neural_percent;

  return (
    <div className="flex flex-col gap-4">
      <Section>
        <SectionHeader title="Azure TTS" />
        <SectionBody>
          <FormField label="訂閱金鑰" htmlFor="azure-subscription-key">
            <div className="flex flex-col gap-2 md:flex-row">
              <Input
                id="azure-subscription-key"
                type={keyVisible ? "text" : "password"}
                value={keyInput}
                placeholder={credStatus.configured ? "已設定金鑰，輸入新金鑰可覆寫" : "輸入 Azure 訂閱金鑰"}
                onChange={(event) => setKeyInput(event.target.value)}
              />
              <Button type="button" variant="secondary" onClick={() => setKeyVisible((value) => !value)}>
                {keyVisible ? "隱藏" : "顯示"}
              </Button>
              <Button
                type="button"
                variant="destructive"
                disabled={!credStatus.configured || saving}
                onClick={() => void deleteCredentials()}
              >
                移除金鑰
              </Button>
            </div>
          </FormField>

          <FormField label="區域" htmlFor="azure-region-select">
            <Select value={region} onValueChange={(value) => setRegion(value)}>
              <SelectTrigger id="azure-region-select">
                <SelectValue placeholder="選擇區域" />
              </SelectTrigger>
              <SelectContent>
                {REGIONS.map((item) => (
                  <SelectItem key={item.id} value={item.id}>
                    {item.label} ({item.id})
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </FormField>

          <FormField label="方案">
            <RadioGroup
              orientation="horizontal"
              value={tier}
              onValueChange={(value) => void switchTier(value === "S0" ? "S0" : "F0")}
              className="grid grid-cols-1 gap-2 sm:grid-cols-2"
            >
              <RadioGroupItem id="billing-f0" value="F0" size="sm" label="F0 (免費)" />
              <RadioGroupItem id="billing-s0" value="S0" size="sm" label="S0 (付費)" />
            </RadioGroup>
          </FormField>

          <div className="flex flex-wrap items-center gap-2">
            <Button
              type="button"
              variant="primary"
              disabled={saving}
              onClick={() => void saveAndTest()}
            >
              儲存並測試
            </Button>
            <Button
              type="button"
              variant="secondary"
              disabled={!credStatus.configured || testStatus === "testing"}
              onClick={() => void testAndLoadVoices()}
            >
              測試連線
            </Button>
          </div>

          <StatusText tone="info" size="sm">
            {credStatus.configured ? `已設定金鑰（${credStatus.region ?? region}）` : "尚未設定金鑰"}
          </StatusText>
          <StatusText tone={testStatus === "error" ? "error" : "info"} size="sm">
            {testStatus === "testing"
              ? "連線測試中..."
              : testStatus === "ok"
                ? "連線成功"
                : testStatus === "error"
                  ? "連線失敗"
                  : "尚未測試連線"}
          </StatusText>
          {testError ? <StatusText tone="error" size="sm">{testError}</StatusText> : null}
        </SectionBody>
      </Section>

      <Section>
        <SectionHeader title="Azure 用量" description={usage.month || "當月統計"} />
        <SectionBody>
          <UsageMeter
            label="Neural 用量"
            used={usage.neural_used}
            limit={usage.neural_limit}
            percent={usage.neural_percent}
            limitInput={tier === "S0" ? (
              <Input
                type="number"
                value={neuralLimitDraft}
                min={1}
                onChange={(event) => setNeuralLimitDraft(event.target.value)}
                onBlur={() => setNeuralLimitDraft(String(parseLimit(neuralLimitDraft) ?? 1))}
              />
            ) : null}
          />

          {tier === "S0" ? (
            <UsageMeter
              label="HD 用量"
              used={usage.hd_used}
              limit={usage.hd_limit}
              percent={usage.hd_percent}
              limitInput={(
                <Input
                  type="number"
                  value={hdLimitDraft}
                  min={1}
                  onChange={(event) => setHdLimitDraft(event.target.value)}
                  onBlur={() => setHdLimitDraft(String(parseLimit(hdLimitDraft) ?? 1))}
                />
              )}
            />
          ) : null}

          <a
            className="inline-flex items-center gap-2 text-sm text-primary hover:underline"
            href="https://portal.azure.com/#view/Microsoft_Azure_ProjectOxford/CognitiveServicesHub/~/SpeechServices"
            target="_blank"
            rel="noreferrer"
          >
            <UsageDonut
              size="md"
              percent={usagePercent}
              tone={usageTone(usagePercent)}
              aria-label="Azure usage donut"
            />
            Azure Portal
          </a>
        </SectionBody>
      </Section>

      <Section>
        <SectionHeader
          title="語音路由"
          description="朗讀速度與音量會同步套用於 OCR 朗讀與語音試聽。"
        />
        <SectionBody>
          <FormField label="朗讀速度">
            <div className="flex items-center gap-3">
              <Slider
                min={0.5}
                max={2.0}
                step={0.05}
                value={[speechRate]}
                onValueChange={(values) => {
                  const next = values[0];
                  if (typeof next === "number") {
                    setSpeechRateState(next);
                  }
                }}
              />
              <span className="w-12 text-right text-sm text-muted-foreground">
                {speechRate.toFixed(2)}x
              </span>
            </div>
          </FormField>

          <FormField label="音量">
            <div className="flex items-center gap-3">
              <Slider
                min={0.5}
                max={2.0}
                step={0.05}
                value={[speechVolume]}
                onValueChange={(values) => {
                  const next = values[0];
                  if (typeof next === "number") {
                    setSpeechVolumeState(next);
                  }
                }}
              />
              <span className="w-12 text-right text-sm text-muted-foreground">
                {speechVolume.toFixed(2)}x
              </span>
            </div>
          </FormField>

          <div className="flex items-center justify-between gap-2">
            <StatusText tone="info" size="sm">
              目前方案：{tier}
            </StatusText>
            <Button
              type="button"
              variant="secondary"
              disabled={!canLoadVoices || loadingVoices}
              onClick={() => void loadVoices()}
            >
              {loadingVoices ? "載入中..." : "重新載入語音"}
            </Button>
          </div>

          <div className="grid gap-3">
            {LANGUAGES.map((item) => {
              const voices = voicesByLang[item.code] ?? [];
              const usableVoices = voices.filter((voice) => isVoiceUsable(voice.id, tier));
              const selected = selectedVoiceFor(item);
              const previewing = previewingLang === item.code;

              return (
                <div key={item.code} className="rounded-md border border-border p-3">
                  <div className="mb-2 flex flex-wrap items-center gap-2">
                    <span className="text-sm font-medium text-foreground">
                      {item.label} ({item.code})
                    </span>
                  </div>

                  <FormField label="語音選擇" htmlFor={`voice-select-${item.code}`}>
                    <div className="flex items-center gap-2">
                      <div className="min-w-0 flex-1">
                    <Select
                      value={selected}
                      onValueChange={(value) => void changeVoice(item.code, value)}
                      disabled={!credStatus.configured || usableVoices.length === 0}
                    >
                      <SelectTrigger id={`voice-select-${item.code}`}>
                        <SelectValue placeholder="請選擇語音" />
                      </SelectTrigger>
                      <SelectContent>
                        {usableVoices.length === 0 ? (
                          <SelectItem value={item.fallback}>{item.fallback}</SelectItem>
                        ) : (
                          usableVoices.map((voice) => (
                            <SelectItem key={voice.id} value={voice.id}>
                              {voiceLabel(voice)}
                            </SelectItem>
                          ))
                        )}
                      </SelectContent>
                    </Select>
                      </div>
                      <Button
                        type="button"
                        variant="secondary"
                        className="h-11 w-28 shrink-0"
                        disabled={!credStatus.configured || previewing || loadingVoices}
                        onClick={() => void handlePreview(item.code)}
                      >
                        {previewing ? "試聽中..." : "試聽"}
                      </Button>
                    </div>
                  </FormField>
                  <StatusText tone="info" size="sm">
                    試聽文字：{item.sample}
                  </StatusText>
                </div>
              );
            })}
          </div>
        </SectionBody>
      </Section>

      {statusMsg ? <StatusText tone="info" size="sm">{statusMsg}</StatusText> : null}
    </div>
  );
}

function UsageMeter({
  label,
  used,
  limit,
  percent,
  limitInput,
}: {
  label: string;
  used: number;
  limit: number;
  percent: number;
  limitInput: ReactNode;
}) {
  return (
    <div className="rounded-md border border-border p-3">
      <ProgressBar
        tone={usageTone(percent)}
        value={used}
        max={limit || 1}
        label={label}
        subLabel={`${formatNumber(used)} / ${formatNumber(limit)} (${percent.toFixed(1)}%)`}
      />
      {limitInput ? (
        <div className="mt-3 grid gap-2 sm:grid-cols-2 sm:items-center">
          <span className="text-sm text-muted-foreground">上限</span>
          {limitInput}
        </div>
      ) : null}
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

function usageTone(percent: number): "green" | "yellow" | "red" | "neutral" {
  if (percent >= 90) return "red";
  if (percent >= 70) return "yellow";
  if (percent >= 0) return "green";
  return "neutral";
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
