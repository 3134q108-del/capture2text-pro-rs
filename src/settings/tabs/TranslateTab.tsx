import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useMemo, useState } from "react";
import {
  Button,
  Card,
  CardContent,
  CardHeader,
  FormField,
  Input,
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
  StatusText,
} from "@/components/ui";
import { cn } from "@/lib/utils";

type Scenario = {
  id: string;
  name: string;
  prompt: string;
  builtin: boolean;
};

type TranslationMode = "Smart" | "Direct";

type LanguageItem = {
  code: string;
  native_name: string;
  english_name: string;
  tier: string;
};

type WindowStatePayload = {
  native_lang?: string;
  target_lang?: string;
};

const EMPTY_SCENARIO: Scenario = {
  id: "",
  name: "",
  prompt: "",
  builtin: false,
};

export default function TranslateTab() {
  const [scenarios, setScenarios] = useState<Scenario[]>([]);
  const [activeId, setActiveId] = useState<string>("default");
  const [selectedId, setSelectedId] = useState<string>("");
  const [draft, setDraft] = useState<Scenario>(EMPTY_SCENARIO);

  const [nativeLang, setNativeLang] = useState("zh-TW");
  const [targetLang, setTargetLang] = useState("en-US");
  const [translationMode, setTranslationMode] = useState<TranslationMode>("Smart");
  const [enabledLangs, setEnabledLangs] = useState<LanguageItem[]>([]);
  const [savingLang, setSavingLang] = useState(false);
  const [saveSuccess, setSaveSuccess] = useState(false);

  const [statusMsg, setStatusMsg] = useState<string>("");

  const selectedScenario = useMemo(
    () => scenarios.find((item) => item.id === selectedId) ?? null,
    [scenarios, selectedId],
  );

  const languageOptions = useMemo(() => {
    if (enabledLangs.length > 0) {
      return enabledLangs;
    }
    return [
      { code: nativeLang, native_name: nativeLang, english_name: nativeLang, tier: "" },
      { code: targetLang, native_name: targetLang, english_name: targetLang, tier: "" },
    ].filter((item, index, arr) => arr.findIndex((x) => x.code === item.code) === index);
  }, [enabledLangs, nativeLang, targetLang]);

  useEffect(() => {
    void refresh();
  }, []);

  useEffect(() => {
    let cancelled = false;
    let offLang: undefined | (() => void);

    listen<string>("output-language-changed", (event) => {
      setTargetLang(event.payload);
    }).then((unlisten) => {
      if (cancelled) {
        unlisten();
        return;
      }
      offLang = unlisten;
    });

    return () => {
      cancelled = true;
      offLang?.();
    };
  }, []);

  useEffect(() => {
    if (!saveSuccess) {
      return;
    }
    const timer = setTimeout(() => {
      setSaveSuccess(false);
      setStatusMsg("");
    }, 3000);
    return () => clearTimeout(timer);
  }, [saveSuccess]);

  async function refresh() {
    try {
      const [list, active, outputLang, state, allLanguages, enabledCodes, mode] = await Promise.all([
        invoke<Scenario[]>("list_scenarios"),
        invoke<string>("get_active_scenario"),
        invoke<string>("get_output_language"),
        invoke<WindowStatePayload>("get_window_state"),
        invoke<LanguageItem[]>("get_languages"),
        invoke<string[]>("get_enabled_langs"),
        invoke<string>("get_translation_mode"),
      ]);

      const enabledSet = new Set(enabledCodes);
      const filtered = allLanguages.filter((item) => enabledSet.has(item.code));

      setScenarios(list);
      setActiveId(active);
      setEnabledLangs(filtered);
      setNativeLang(state.native_lang ?? "zh-TW");
      setTargetLang(state.target_lang ?? outputLang ?? "en-US");
      setTranslationMode(mode === "Direct" ? "Direct" : "Smart");

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
    } catch (error) {
      setStatusMsg(String(error));
    }
  }

  async function changeTranslationMode(next: TranslationMode) {
    setTranslationMode(next);
    try {
      await invoke("set_translation_mode", { mode: next });
      setStatusMsg("");
    } catch (error) {
      setStatusMsg(String(error));
    }
  }

  async function saveLanguagePreferences() {
    setSavingLang(true);
    setSaveSuccess(false);
    try {
      const currentEnabled = await invoke<string[]>("get_enabled_langs");
      const enabled = Array.from(new Set([...currentEnabled, nativeLang, targetLang]));
      await invoke("set_language_preferences", {
        nativeLang,
        targetLang,
        enabledLangs: enabled,
      });
      setStatusMsg("✅ 語言設定已儲存");
      setSaveSuccess(true);
    } catch (error) {
      setStatusMsg(String(error));
      setSaveSuccess(false);
    } finally {
      setSavingLang(false);
    }
  }

  function selectScenario(id: string) {
    const selected = scenarios.find((item) => item.id === id);
    if (!selected) {
      return;
    }
    setSelectedId(id);
    setDraft({ ...selected });
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
    setStatusMsg("已新增情境草稿");
  }

  async function saveScenario() {
    const id = draft.id.trim();
    const name = draft.name.trim();
    if (!id) {
      setStatusMsg("Scenario ID 不可為空");
      return;
    }
    if (!name) {
      setStatusMsg("Scenario 名稱不可為空");
      return;
    }

    try {
      await invoke("save_scenario", {
        scenario: { ...draft, id, name, prompt: draft.prompt },
      });
      await refresh();
      setSelectedId(id);
      setStatusMsg("情境已儲存");
    } catch (error) {
      setStatusMsg(String(error));
    }
  }

  async function deleteScenario() {
    if (!selectedScenario) {
      return;
    }
    try {
      await invoke("delete_scenario", { id: selectedScenario.id });
      await refresh();
      setStatusMsg("情境已刪除");
    } catch (error) {
      setStatusMsg(String(error));
    }
  }

  async function applyActiveScenario() {
    const id = draft.id.trim();
    if (!id) {
      return;
    }
    try {
      await invoke("set_active_scenario", { id });
      await refresh();
      setStatusMsg("已套用使用中的情境");
    } catch (error) {
      setStatusMsg(String(error));
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <Section>
        <SectionHeader title="語言設定" />
        <SectionBody>
          <FormField label="翻譯模式" htmlFor="translation-mode-radio" required>
            <RadioGroup
              id="translation-mode-radio"
              orientation="horizontal"
              value={translationMode}
              onValueChange={(value) => void changeTranslationMode(value as TranslationMode)}
              className="gap-4"
            >
              <RadioGroupItem id="mode-smart" value="Smart" size="sm" label="智慧對翻" />
              <RadioGroupItem id="mode-direct" value="Direct" size="sm" label="直接翻譯" />
            </RadioGroup>
          </FormField>

          <div className="grid gap-3 sm:grid-cols-2">
            <FormField label="母語" htmlFor="native-lang-select" required>
              <Select value={nativeLang} onValueChange={(value) => setNativeLang(value)}>
                <SelectTrigger id="native-lang-select">
                  <SelectValue placeholder="請選擇母語" />
                </SelectTrigger>
                <SelectContent>
                  {languageOptions.map((item) => (
                    <SelectItem key={`native-${item.code}`} value={item.code}>
                      {`${item.native_name} (${item.english_name})`}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </FormField>

            <FormField label="目標語言" htmlFor="target-lang-select" required>
              <Select value={targetLang} onValueChange={(value) => setTargetLang(value)}>
                <SelectTrigger id="target-lang-select">
                  <SelectValue placeholder="請選擇目標語言" />
                </SelectTrigger>
                <SelectContent>
                  {languageOptions.map((item) => (
                    <SelectItem key={`target-${item.code}`} value={item.code}>
                      {`${item.native_name} (${item.english_name})`}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </FormField>
          </div>

          <StatusText tone="info" size="sm">
            {translationMode === "Smart" ? (
              <>
                智慧對翻：
                框到母語 → 翻成目標語言（練習）；
                框到其他語言 → 翻成母語（看懂）
              </>
            ) : (
              <>
                直接翻譯：
                不論原文語言，一律翻譯成目標語言。如果原文已是目標語言，模型可能回原文不變。
              </>
            )}
          </StatusText>

          <div className="flex items-center gap-2">
            <Button
              type="button"
              variant="primary"
              onClick={() => void saveLanguagePreferences()}
              aria-disabled={savingLang}
              disabled={savingLang}
            >
              {savingLang ? "儲存中..." : saveSuccess ? "✅ 已儲存" : "儲存語言設定"}
            </Button>
            {statusMsg ? (
              <StatusText tone={saveSuccess ? "success" : "info"} size="sm">
                {statusMsg}
              </StatusText>
            ) : null}
          </div>
        </SectionBody>
      </Section>

      <Section>
        <SectionHeader title="翻譯情境" description="可針對 OCR 翻譯流程自訂 Prompt。" />
        <SectionBody>
          <div className="grid gap-4 sm:grid-cols-4">
            <Card className="sm:col-span-1">
              <CardHeader className="gap-3 p-3">
                <Button type="button" variant="secondary" onClick={createScenario}>
                  新增情境
                </Button>
              </CardHeader>
              <CardContent className="p-3 pt-0">
                <ul className="flex max-h-96 flex-col gap-2 overflow-y-auto">
                  {scenarios.map((item) => {
                    const isActive = item.id === selectedId;
                    return (
                      <li key={item.id}>
                        <Button
                          type="button"
                          variant={isActive ? "secondary" : "ghost"}
                          className={cn(
                            "h-auto w-full justify-between border px-3 py-2",
                            isActive ? "border-primary" : "border-border",
                          )}
                          onClick={() => selectScenario(item.id)}
                        >
                          <span className="truncate text-left">{item.name}</span>
                          <span className="flex shrink-0 items-center gap-1 text-xs text-muted-foreground">
                            {item.builtin ? <span>內建</span> : null}
                            {item.id === activeId ? <span>使用中</span> : null}
                          </span>
                        </Button>
                      </li>
                    );
                  })}
                </ul>
              </CardContent>
            </Card>

            <Card className="sm:col-span-3 sm:max-w-3xl">
              <CardContent className="flex flex-col gap-4 p-4">
                <FormField label="情境名稱" htmlFor="scenario-name" required>
                  <Input
                    id="scenario-name"
                    value={draft.name}
                    onChange={(event) => setDraft((prev) => ({ ...prev, name: event.target.value }))}
                  />
                </FormField>

                <FormField label="提示內容" htmlFor="scenario-prompt">
                  <textarea
                    id="scenario-prompt"
                    value={draft.prompt}
                    onChange={(event) => setDraft((prev) => ({ ...prev, prompt: event.target.value }))}
                    className="min-h-72 w-full rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                  />
                </FormField>

                <div className="flex flex-wrap items-center gap-2">
                  <Button type="button" variant="primary" onClick={() => void saveScenario()}>
                    儲存
                  </Button>
                  <Button type="button" variant="secondary" onClick={() => void applyActiveScenario()}>
                    設為使用中
                  </Button>
                  <Button
                    type="button"
                    variant="destructive"
                    disabled={Boolean(selectedScenario?.builtin)}
                    onClick={() => void deleteScenario()}
                  >
                    刪除
                  </Button>
                </div>

                <StatusText tone="info" size="sm">
                  情境會影響 OCR 翻譯流程中送給模型的 Prompt。
                </StatusText>
                {statusMsg ? (
                  <StatusText tone="info" size="sm">
                    {statusMsg}
                  </StatusText>
                ) : null}
              </CardContent>
            </Card>
          </div>
        </SectionBody>
      </Section>
    </div>
  );
}