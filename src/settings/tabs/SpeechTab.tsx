import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";
import {
  deleteAzureCredentials,
  getAzureCredentialsStatus,
  getVoiceRouting,
  listAzureVoices,
  previewVoice,
  saveAzureCredentials,
  setVoiceRouting,
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

  const [enabledLanguages, setEnabledLanguages] = useState<LanguageItem[]>([]);
  const [voicesByLang, setVoicesByLang] = useState<Record<string, AzureVoice[]>>({});
  const [routing, setRouting] = useState<Record<string, string>>({});

  const canOperate = useMemo(() => credStatus.configured && testStatus !== "testing", [credStatus, testStatus]);

  useEffect(() => {
    void refreshInitial();
  }, []);

  async function refreshInitial() {
    try {
      const [status, route, allLanguages, enabled] = await Promise.all([
        getAzureCredentialsStatus(),
        getVoiceRouting(),
        invoke<LanguageItem[]>("get_languages"),
        invoke<string[]>("get_enabled_langs"),
      ]);
      setCredStatus(status);
      setRouting(route);
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

  async function saveAndTest() {
    const key = keyInput.trim();
    if (!key) {
      setStatusMsg("Azure subscription key is required.");
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
      setStatusMsg("Azure credentials deleted.");
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
      setStatusMsg(`${lang} voice updated.`);
    } catch (error) {
      setStatusMsg(String(error));
    }
  }

  async function handlePreview(lang: string, voiceId: string) {
    try {
      setPreviewingLang(lang);
      await previewVoice(lang, voiceId);
    } catch (error) {
      setStatusMsg(`Preview failed: ${String(error)}`);
    } finally {
      setPreviewingLang(null);
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
        <SectionHeader title="Azure TTS" />
        <SectionBody>
          <FormField label="Subscription Key" htmlFor="azure-subscription-key">
            <Input
              id="azure-subscription-key"
              type="password"
              value={keyInput}
              placeholder={credStatus.configured ? "Enter new key to replace existing one" : "Enter Azure subscription key"}
              onChange={(event) => setKeyInput(event.target.value)}
            />
          </FormField>

          <FormField label="Region" htmlFor="azure-region-select">
            <Select value={region} onValueChange={(value) => setRegion(value)}>
              <SelectTrigger id="azure-region-select">
                <SelectValue placeholder="Select region" />
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
              Save & Test
            </Button>
            <Button type="button" variant="secondary" disabled={!credStatus.configured} onClick={() => void testAndLoadVoices()}>
              Test
            </Button>
            <Button type="button" variant="destructive" disabled={!credStatus.configured || saving} onClick={() => void deleteCredentials()}>
              Delete
            </Button>
          </div>

          <StatusText tone="info" size="sm">
            {credStatus.configured ? `Configured (${credStatus.region ?? region})` : "Not configured"}
          </StatusText>
          {testError ? <StatusText tone="error" size="sm">{testError}</StatusText> : null}
        </SectionBody>
      </Section>

      <Section>
        <SectionHeader title="Voice Routing" description="Only enabled languages are shown here." />
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
                  <FormField label="Voice" htmlFor={`voice-select-${item.code}`}>
                    <div className="flex items-center gap-2">
                      <div className="min-w-0 flex-1">
                        <Select
                          value={selected}
                          onValueChange={(value) => void changeVoice(item.code, value)}
                          disabled={!canOperate}
                        >
                          <SelectTrigger id={`voice-select-${item.code}`}>
                            <SelectValue placeholder="Select voice" />
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
                      <Button
                        type="button"
                        variant="secondary"
                        className="h-11 w-24 shrink-0"
                        disabled={!canOperate || previewingLang === item.code}
                        onClick={() => void handlePreview(item.code, selected)}
                      >
                        {previewingLang === item.code ? "..." : "Preview"}
                      </Button>
                    </div>
                  </FormField>
                  {fallbackTier ? (
                    <StatusText tone="info" size="sm">
                      Tier {item.tier}: fallback voice may have weaker quality.
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
