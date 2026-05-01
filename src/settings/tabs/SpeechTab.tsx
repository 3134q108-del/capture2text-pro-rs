import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useMemo, useState } from "react";
import {
  deleteAzureCredentials,
  getAzureCredentialsStatus,
  getSpeechRate,
  getSpeechVolume,
  getVoiceRouting,
  listAzureVoices,
  previewVoice,
  saveAzureCredentials,
  setSpeechRate,
  setSpeechVolume,
  setVoiceRouting,
  stopSpeaking,
  testAzureConnection,
  type AzureCredentialsStatus,
  type AzureVoice,
} from "../../services/tts";
import {
  Button,
  FormField,
  Input,
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
} from "@/components/ui";

type Tier = "S" | "A" | "B" | "C";
type TestStatus = "idle" | "testing" | "ok" | "error";

type LanguageItem = {
  code: string;
  native_name: string;
  english_name: string;
  tier: Tier;
};

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

const SA_ALTERNATIVES: Record<string, string[]> = {
  "zh-TW": ["zh-TW-HsiaoChenNeural", "zh-TW-YunJheNeural"],
  "zh-CN": ["zh-CN-XiaoxiaoNeural", "zh-CN-YunyangNeural"],
  "en-US": ["en-US-AvaNeural", "en-US-JennyNeural"],
  "ja-JP": ["ja-JP-NanamiNeural", "ja-JP-KeitaNeural"],
  "ko-KR": ["ko-KR-SunHiNeural", "ko-KR-InJoonNeural"],
  "fr-FR": ["fr-FR-VivienneMultilingualNeural", "fr-FR-DeniseNeural"],
  "de-DE": ["de-DE-SeraphinaMultilingualNeural", "de-DE-KatjaNeural"],
  "es-ES": ["es-ES-XimenaNeural", "es-ES-ElviraNeural"],
  "pt-PT": ["pt-PT-RaquelNeural", "pt-PT-DuarteNeural"],
  "it-IT": ["it-IT-IsabellaNeural", "it-IT-DiegoNeural"],
  "ru-RU": ["ru-RU-SvetlanaNeural", "ru-RU-DmitryNeural"],
  "vi-VN": ["vi-VN-HoaiMyNeural", "vi-VN-NamMinhNeural"],
};

export default function SpeechTab() {
  const [keyInput, setKeyInput] = useState("");
  const [region, setRegion] = useState("eastasia");
  const [credStatus, setCredStatus] = useState<AzureCredentialsStatus>(EMPTY_STATUS);
  const [testStatus, setTestStatus] = useState<TestStatus>("idle");
  const [testError, setTestError] = useState("");
  const [statusMsg, setStatusMsg] = useState("");
  const [saving, setSaving] = useState(false);
  const [previewingLang, setPreviewingLang] = useState<string | null>(null);
  const [speechRate, setSpeechRateState] = useState(1.0);
  const [speechVolume, setSpeechVolumeState] = useState(1.0);

  const [enabledLanguages, setEnabledLanguages] = useState<LanguageItem[]>([]);
  const [voicesByLang, setVoicesByLang] = useState<Record<string, AzureVoice[]>>({});
  const [routing, setRouting] = useState<Record<string, string>>({});

  const canOperate = useMemo(() => credStatus.configured && testStatus !== "testing", [credStatus, testStatus]);

  useEffect(() => {
    void refreshInitial();
  }, []);

  async function refreshInitial() {
    try {
      const [status, route, allLanguages, enabled, rate, volume] = await Promise.all([
        getAzureCredentialsStatus(),
        getVoiceRouting(),
        invoke<LanguageItem[]>("get_languages"),
        invoke<string[]>("get_enabled_langs"),
        getSpeechRate(),
        getSpeechVolume(),
      ]);
      setCredStatus(status);
      setRouting(route);
      setSpeechRateState(rate);
      setSpeechVolumeState(volume);
      if (status.region) {
        setRegion(status.region);
      }
      const enabledSet = new Set(enabled);
      setEnabledLanguages(allLanguages.filter((item) => enabledSet.has(item.code)));
      if (status.configured) {
        await loadVoices(allLanguages.filter((item) => enabledSet.has(item.code)));
      }
    } catch (error) {
      setStatusMsg(String(error));
    }
  }

  useEffect(() => {
    let cancelled = false;
    let unlisten: undefined | (() => void);
    listen<{ target?: string }>("tts-done", () => {
      setPreviewingLang(null);
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlisten = fn;
      }
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  async function saveAndTest() {
    const key = keyInput.trim();
    if (!key && credStatus.configured) {
      await testAndLoadVoices();
      return;
    }
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
      setStatusMsg("Azure 認證已刪除。");
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
      await testAzureConnection();
      setTestStatus("ok");
      const status = await getAzureCredentialsStatus();
      setCredStatus(status);
      await loadVoices(enabledLanguages);
    } catch (error) {
      setTestStatus("error");
      setTestError(String(error));
    }
  }

  async function loadVoices(languages: LanguageItem[]) {
    const entries = await Promise.all(
      languages.map(async (item) => [item.code, await listAzureVoices(item.code)] as const),
    );
    setVoicesByLang(Object.fromEntries(entries));
  }

  async function changeVoice(lang: string, voiceId: string) {
    try {
      await setVoiceRouting(lang, voiceId);
      setRouting((prev) => ({ ...prev, [lang]: voiceId }));
      setStatusMsg(`${lang} 音色已更新。`);
    } catch (error) {
      setStatusMsg(String(error));
    }
  }

  async function handlePreview(lang: string, voiceId: string) {
    try {
      setPreviewingLang(lang);
      await previewVoice(lang, voiceId);
    } catch (error) {
      setStatusMsg(`試聽失敗：${String(error)}`);
      setPreviewingLang(null);
    }
  }

  async function handleStopPreview() {
    try {
      await stopSpeaking();
    } catch (error) {
      setStatusMsg(`停止失敗：${String(error)}`);
    } finally {
      setPreviewingLang(null);
    }
  }

  async function handleRateChange(value: number) {
    setSpeechRateState(value);
    try {
      await setSpeechRate(value);
    } catch (error) {
      setStatusMsg(`朗讀速度更新失敗：${String(error)}`);
    }
  }

  async function handleVolumeChange(value: number) {
    setSpeechVolumeState(value);
    try {
      await setSpeechVolume(value);
    } catch (error) {
      setStatusMsg(`音量更新失敗：${String(error)}`);
    }
  }

  function voiceOptions(item: LanguageItem): string[] {
    const defaults = [defaultVoiceFor(item), ...(SA_ALTERNATIVES[item.code] ?? [])];
    const fromApi = (voicesByLang[item.code] ?? []).map((voice) => voice.id);
    const merged = [...defaults, ...fromApi];
    return merged.filter((id, index) => merged.indexOf(id) === index);
  }

  function defaultVoiceFor(item: LanguageItem): string {
    const preferred = SA_ALTERNATIVES[item.code]?.[0];
    return preferred ?? "en-US-AvaNeural";
  }

  return (
    <div className="flex flex-col gap-4">
      <Section>
        <SectionHeader title="Azure 語音合成" />
        <SectionBody>
          <FormField label="訂閱金鑰" htmlFor="azure-subscription-key">
            <Input
              id="azure-subscription-key"
              type="password"
              value={keyInput}
              placeholder={credStatus.configured ? "輸入新金鑰以取代現有金鑰" : "輸入 Azure 訂閱金鑰"}
              onChange={(event) => setKeyInput(event.target.value)}
            />
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

          <div className="flex flex-wrap items-center gap-2">
            <Button type="button" variant="primary" disabled={saving} onClick={() => void saveAndTest()}>
              儲存並測試
            </Button>
            <Button type="button" variant="secondary" disabled={!credStatus.configured} onClick={() => void testAndLoadVoices()}>
              測試
            </Button>
            <Button type="button" variant="destructive" disabled={!credStatus.configured || saving} onClick={() => void deleteCredentials()}>
              刪除
            </Button>
          </div>

          <StatusText tone="info" size="sm">
            {credStatus.configured ? `已設定 (${credStatus.region ?? region})` : "未設定"}
          </StatusText>
          {testError ? <StatusText tone="error" size="sm">{testError}</StatusText> : null}
        </SectionBody>
      </Section>

      <Section>
        <SectionHeader title="朗讀控制" description="調整 OCR 結果朗讀的速度與音量。" />
        <SectionBody>
          <FormField label={`朗讀速度（${speechRate.toFixed(2)}x）`} htmlFor="speech-rate-slider">
            <Slider
              id="speech-rate-slider"
              value={[speechRate]}
              onValueChange={(v) => void handleRateChange(v[0])}
              min={0.5}
              max={2.0}
              step={0.05}
            />
          </FormField>
          <FormField label={`音量（${(speechVolume * 100).toFixed(0)}%）`} htmlFor="speech-volume-slider">
            <Slider
              id="speech-volume-slider"
              value={[speechVolume]}
              onValueChange={(v) => void handleVolumeChange(v[0])}
              min={0.5}
              max={2.0}
              step={0.05}
            />
          </FormField>
        </SectionBody>
      </Section>

      <Section>
        <SectionHeader title="音色路由" description="僅顯示在語言 tab 中啟用的語言。" />
        <SectionBody>
          <div className="grid gap-3">
            {enabledLanguages.map((item) => {
              const options = voiceOptions(item);
              const selected = routing[item.code] ?? defaultVoiceFor(item);
              const fallbackTier = item.tier === "B" || item.tier === "C";
              return (
                <div key={item.code} className="rounded-md border border-border p-3">
                  <div className="mb-2 text-sm font-medium text-foreground">
                    {item.native_name} ({item.english_name}) - {item.code}
                  </div>
                  <FormField label="音色" htmlFor={`voice-select-${item.code}`}>
                    <div className="flex items-center gap-2">
                      <div className="min-w-0 flex-1">
                        <Select
                          value={selected}
                          onValueChange={(value) => void changeVoice(item.code, value)}
                          disabled={!canOperate}
                        >
                          <SelectTrigger id={`voice-select-${item.code}`}>
                            <SelectValue placeholder="選擇音色" />
                          </SelectTrigger>
                          <SelectContent>
                            {options.map((voiceId) => (
                              <SelectItem key={voiceId} value={voiceId}>
                                {voiceId}
                              </SelectItem>
                            ))}
                          </SelectContent>
                        </Select>
                      </div>
                      {previewingLang === item.code ? (
                        <Button
                          type="button"
                          variant="destructive"
                          className="h-11 w-24 shrink-0"
                          onClick={() => void handleStopPreview()}
                        >
                          停止
                        </Button>
                      ) : (
                        <Button
                          type="button"
                          variant="secondary"
                          className="h-11 w-24 shrink-0"
                          disabled={!canOperate || previewingLang !== null}
                          onClick={() => void handlePreview(item.code, selected)}
                        >
                          試聽
                        </Button>
                      )}
                    </div>
                  </FormField>
                  {fallbackTier ? (
                    <StatusText tone="info" size="sm">
                      {item.tier === "B"
                        ? "進階語言：fallback 音色品質較低，可能不符合該語言發音"
                        : "實驗語言：走英文 fallback 音色，僅供測試"}
                    </StatusText>
                  ) : null}
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
