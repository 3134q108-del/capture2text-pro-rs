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
  StatusText,
} from "@/components/ui";
import { cn } from "@/lib/utils";

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

function normalizeLang(value: string): OutputLang {
  return (
    ["zh-TW", "zh-CN", "en-US", "ja-JP", "ko-KR", "de-DE", "fr-FR"] as const
  ).includes(value as OutputLang)
    ? (value as OutputLang)
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
    let cancelled = false;
    let offLang: undefined | (() => void);

    listen<string>("output-language-changed", (event) => {
      setOutputLang(normalizeLang(event.payload));
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
    } catch (error) {
      setStatusMsg(String(error));
    }
  }

  async function changeOutputLang(next: OutputLang) {
    try {
      await invoke("set_output_language", { lang: next });
      setOutputLang(next);
      setStatusMsg("輸出語言已更新");
    } catch (error) {
      setStatusMsg(String(error));
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
      name: "新情境",
      prompt: "",
      builtin: false,
    };
    setSelectedId(id);
    setDraft(next);
    setStatusMsg("已建立新情境，請填入內容");
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
      setStatusMsg("已設為目前使用情境");
    } catch (error) {
      setStatusMsg(String(error));
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <Section>
        <SectionHeader title="輸出語言" />
        <SectionBody>
          <RadioGroup
            orientation="vertical"
            value={outputLang}
            onValueChange={(value) => void changeOutputLang(normalizeLang(value))}
            className="flex flex-wrap gap-x-4 gap-y-2"
          >
            {LANG_OPTIONS.map((option) => (
              <RadioGroupItem
                key={option.code}
                id={`output-lang-${option.code}`}
                value={option.code}
                size="sm"
                label={option.label}
              />
            ))}
          </RadioGroup>
        </SectionBody>
      </Section>

      <Section>
        <SectionHeader
          title="翻譯情境"
          description="情境會影響 OCR 結果的翻譯語氣與格式。"
        />
        <SectionBody>
          <div className="grid gap-4 lg:grid-cols-4">
            <Card className="lg:col-span-1">
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

            <Card className="lg:col-span-3 lg:max-w-3xl">
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
                  新增情境後，請在 Result 視窗重新翻譯以套用內容。
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
