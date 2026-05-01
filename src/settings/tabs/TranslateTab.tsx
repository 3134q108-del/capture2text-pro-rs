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

type LanguageCode = "zh-TW" | "zh-CN" | "en-US" | "ja-JP" | "ko-KR";

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

const LANG_OPTIONS: { code: LanguageCode; label: string }[] = [
  { code: "zh-TW", label: "繁體中文" },
  { code: "zh-CN", label: "简体中文" },
  { code: "en-US", label: "English" },
  { code: "ja-JP", label: "日本語" },
  { code: "ko-KR", label: "한국어" },
];

function normalizeLang(value: string): LanguageCode {
  return (
    ["zh-TW", "zh-CN", "en-US", "ja-JP", "ko-KR"] as const
  ).includes(value as LanguageCode)
    ? (value as LanguageCode)
    : "zh-TW";
}

export default function TranslateTab() {
  const [scenarios, setScenarios] = useState<Scenario[]>([]);
  const [activeId, setActiveId] = useState<string>("default");
  const [selectedId, setSelectedId] = useState<string>("");
  const [draft, setDraft] = useState<Scenario>(EMPTY_SCENARIO);

  const [nativeLang, setNativeLang] = useState<LanguageCode>("zh-TW");
  const [targetLang, setTargetLang] = useState<LanguageCode>("en-US");
  const [savingLang, setSavingLang] = useState(false);

  const [statusMsg, setStatusMsg] = useState<string>("");
  const hasSameLang = nativeLang === targetLang;

  const selectedScenario = useMemo(
    () => scenarios.find((item) => item.id === selectedId) ?? null,
    [scenarios, selectedId],
  );

  useEffect(() => {
    void refresh();
  }, []);

  useEffect(() => {
    let cancelled = false;
    let offLang: undefined | (() => void);

    listen<string>("output-language-changed", (event) => {
      setTargetLang(normalizeLang(event.payload));
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

  async function refresh() {
    try {
      const [list, active, outputLang, state] = await Promise.all([
        invoke<Scenario[]>("list_scenarios"),
        invoke<string>("get_active_scenario"),
        invoke<string>("get_output_language"),
        invoke<WindowStatePayload>("get_window_state"),
      ]);

      setScenarios(list);
      setActiveId(active);
      setNativeLang(normalizeLang(state.native_lang ?? "zh-TW"));
      setTargetLang(normalizeLang(state.target_lang ?? outputLang));

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

  async function saveLanguagePreferences() {
    if (hasSameLang) {
      return;
    }

    setSavingLang(true);
    try {
      try {
        await invoke("set_language_preferences", {
          nativeLang,
          targetLang,
          enabledLangs: LANG_OPTIONS.map((item) => item.code),
        });
      } catch {
        await invoke("set_output_language", { lang: targetLang });
      }
      setStatusMsg("語言設定已儲存");
    } catch (error) {
      setStatusMsg(String(error));
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
    setStatusMsg("已建立新情境草稿");
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
      setStatusMsg("已套用為目前情境");
    } catch (error) {
      setStatusMsg(String(error));
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <Section>
        <SectionHeader title="語言設定" />
        <SectionBody>
          <div className="grid gap-3 sm:grid-cols-2">
            <FormField label="母語" htmlFor="native-lang-select" required>
              <Select value={nativeLang} onValueChange={(value) => setNativeLang(normalizeLang(value))}>
                <SelectTrigger id="native-lang-select">
                  <SelectValue placeholder="選擇母語" />
                </SelectTrigger>
                <SelectContent>
                  {LANG_OPTIONS.map((option) => (
                    <SelectItem key={`native-${option.code}`} value={option.code}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </FormField>

            <FormField label="目標語言" htmlFor="target-lang-select" required>
              <Select value={targetLang} onValueChange={(value) => setTargetLang(normalizeLang(value))}>
                <SelectTrigger id="target-lang-select">
                  <SelectValue placeholder="選擇目標語言" />
                </SelectTrigger>
                <SelectContent>
                  {LANG_OPTIONS.map((option) => (
                    <SelectItem key={`target-${option.code}`} value={option.code}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </FormField>
          </div>

          {hasSameLang ? (
            <StatusText tone="info" size="sm">
              母語與目標語言不能相同，請調整後再儲存。
            </StatusText>
          ) : null}

          <StatusText tone="info" size="sm">
            智慧對翻:
            框到母語 → 翻成目標語言(練習)
            框到其他語言 → 翻成母語(看懂)
          </StatusText>

          <div>
            <Button
              type="button"
              variant="primary"
              onClick={() => void saveLanguagePreferences()}
              aria-disabled={hasSameLang || savingLang}
              disabled={hasSameLang || savingLang}
            >
              儲存語言設定
            </Button>
          </div>
        </SectionBody>
      </Section>

      <Section>
        <SectionHeader title="翻譯情境" description="可維護 OCR 翻譯時使用的 Prompt。" />
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
                            {item.builtin ? <span>Built-in</span> : null}
                            {item.id === activeId ? <span>Active</span> : null}
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

                <FormField label="Prompt" htmlFor="scenario-prompt">
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
                    設為啟用
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
                  情境會在 OCR 翻譯時生效，可依場景調整 Prompt。
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
