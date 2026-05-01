import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";
import { Lock } from "lucide-react";
import { Button, Card, CardContent, Checkbox, Section, SectionBody, SectionHeader, StatusText } from "@/components/ui";

type Tier = "S" | "A" | "B" | "C";

type LanguageItem = {
  code: string;
  native_name: string;
  english_name: string;
  tier: Tier;
};

type WindowStatePayload = {
  native_lang?: string;
  target_lang?: string;
};

const DEFAULT_ENABLED = ["zh-CN", "zh-TW", "en-US", "ja-JP", "ko-KR"];
const TIERS: Tier[] = ["S", "A", "B", "C"];

export default function LanguagesTab() {
  const [languages, setLanguages] = useState<LanguageItem[]>([]);
  const [enabledSet, setEnabledSet] = useState<Set<string>>(new Set(DEFAULT_ENABLED));
  const [nativeLang, setNativeLang] = useState("zh-TW");
  const [targetLang, setTargetLang] = useState("en-US");
  const [collapsed, setCollapsed] = useState<Record<Tier, boolean>>({
    S: false,
    A: false,
    B: false,
    C: false,
  });
  const [saving, setSaving] = useState(false);
  const [statusMsg, setStatusMsg] = useState("");

  const lockedCodes = useMemo(() => new Set([nativeLang, targetLang]), [nativeLang, targetLang]);

  const grouped = useMemo(() => {
    return TIERS.map((tier) => ({
      tier,
      items: languages.filter((lang) => lang.tier === tier),
    }));
  }, [languages]);

  useEffect(() => {
    void refresh();
  }, []);

  async function refresh() {
    try {
      const [allLanguages, enabled, ws] = await Promise.all([
        invoke<LanguageItem[]>("get_languages"),
        invoke<string[]>("get_enabled_langs"),
        invoke<WindowStatePayload>("get_window_state"),
      ]);
      setLanguages(allLanguages);
      setEnabledSet(new Set(enabled));
      setNativeLang(ws.native_lang ?? "zh-TW");
      setTargetLang(ws.target_lang ?? "en-US");
      setStatusMsg("");
    } catch (error) {
      setStatusMsg(String(error));
    }
  }

  function setBatchEnabled(codes: string[]) {
    const next = new Set<string>([...codes, ...lockedCodes]);
    setEnabledSet(next);
  }

  function selectTierS() {
    const tierCodes = languages.filter((item) => item.tier === "S").map((item) => item.code);
    setBatchEnabled(tierCodes);
  }

  function selectTierSA() {
    const tierCodes = languages
      .filter((item) => item.tier === "S" || item.tier === "A")
      .map((item) => item.code);
    setBatchEnabled(tierCodes);
  }

  function selectAll() {
    setBatchEnabled(languages.map((item) => item.code));
  }

  function resetDefault() {
    setBatchEnabled(DEFAULT_ENABLED);
  }

  function toggleLang(code: string, checked: boolean) {
    if (lockedCodes.has(code)) {
      return;
    }
    const next = new Set(enabledSet);
    if (checked) {
      next.add(code);
    } else {
      next.delete(code);
    }
    setEnabledSet(next);
  }

  function toggleTierCollapse(tier: Tier) {
    setCollapsed((prev) => ({ ...prev, [tier]: !prev[tier] }));
  }

  async function savePreferences() {
    const enabled = languages.filter((item) => enabledSet.has(item.code)).map((item) => item.code);
    setSaving(true);
    try {
      await invoke("set_language_preferences", {
        nativeLang,
        targetLang,
        enabledLangs: enabled,
      });
      setStatusMsg("語言啟用設定已儲存");
      await refresh();
    } catch (error) {
      setStatusMsg(String(error));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <Section>
        <SectionHeader title="啟用語言" description="選擇在翻譯與語音頁顯示的語言清單。" />
        <SectionBody>
          <div className="flex flex-wrap gap-2">
            <Button type="button" variant="secondary" size="sm" onClick={selectTierS}>
              全選 Tier S
            </Button>
            <Button type="button" variant="secondary" size="sm" onClick={selectTierSA}>
              全選 Tier S+A
            </Button>
            <Button type="button" variant="secondary" size="sm" onClick={selectAll}>
              全勾
            </Button>
            <Button type="button" variant="secondary" size="sm" onClick={resetDefault}>
              還原預設
            </Button>
          </div>
        </SectionBody>
      </Section>

      {grouped.map((group) => (
        <Card key={group.tier}>
          <CardContent className="p-4">
            <div className="mb-3 flex items-center justify-between">
              <Button type="button" variant="ghost" size="sm" onClick={() => toggleTierCollapse(group.tier)}>
                {collapsed[group.tier] ? "展開" : "收合"} Tier {group.tier} ({group.items.length})
              </Button>
            </div>
            {!collapsed[group.tier] ? (
              <div className="grid gap-2 sm:grid-cols-2">
                {group.items.map((item) => {
                  const checked = enabledSet.has(item.code);
                  const locked = lockedCodes.has(item.code);
                  return (
                    <div key={item.code} className="rounded-md border border-border p-2">
                      <Checkbox
                        checked={checked}
                        onCheckedChange={(value) => toggleLang(item.code, value === true)}
                        disabled={locked}
                        label={(
                          <span className="inline-flex items-center gap-2">
                            <span>{item.native_name}</span>
                            <span className="text-xs text-muted-foreground">({item.english_name})</span>
                            <span className="rounded border border-border px-1.5 py-0.5 text-xs">Tier {item.tier}</span>
                            {locked ? <Lock className="h-3.5 w-3.5" aria-hidden="true" /> : null}
                          </span>
                        )}
                        description={locked ? "目前母語或目標語言，無法取消。" : item.code}
                        title={locked ? "目前母語或目標語言，無法取消" : undefined}
                      />
                    </div>
                  );
                })}
              </div>
            ) : null}
          </CardContent>
        </Card>
      ))}

      <div className="flex items-center gap-2">
        <Button type="button" variant="primary" onClick={() => void savePreferences()} disabled={saving}>
          儲存語言設定
        </Button>
      </div>
      {statusMsg ? <StatusText tone="info" size="sm">{statusMsg}</StatusText> : null}
    </div>
  );
}
