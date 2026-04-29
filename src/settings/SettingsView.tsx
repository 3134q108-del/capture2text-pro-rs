import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { Banner, Button, TabNav, TabNavContent, TabNavList, TabNavTrigger } from "@/components/ui";
import AboutTab from "./tabs/AboutTab";
import OutputTab from "./tabs/OutputTab";
import SpeechTab from "./tabs/SpeechTab";
import TranslateTab from "./tabs/TranslateTab";

type HealthWarning = {
  status: string;
  message: string;
};

type TabKey = "translate" | "speech" | "output" | "about";

const TAB_ITEMS: Array<{ key: TabKey; label: string }> = [
  { key: "translate", label: "翻譯" },
  { key: "speech", label: "語音" },
  { key: "output", label: "輸出" },
  { key: "about", label: "關於" },
];

function isTabKey(value: string): value is TabKey {
  return value === "translate" || value === "speech" || value === "output" || value === "about";
}

export default function SettingsView() {
  const [activeTab, setActiveTab] = useState<TabKey>("translate");
  const [healthWarning, setHealthWarning] = useState<HealthWarning | null>(null);

  useEffect(() => {
    let cancelled = false;
    let offHealth: undefined | (() => void);
    let offNav: undefined | (() => void);

    listen<HealthWarning>("health-warning", (event) => {
      setHealthWarning(event.payload);
    }).then((unlisten) => {
      if (cancelled) {
        unlisten();
        return;
      }
      offHealth = unlisten;
    });

    listen<string>("settings-navigate", (event) => {
      if (isTabKey(event.payload)) {
        setActiveTab(event.payload);
      }
    }).then((unlisten) => {
      if (cancelled) {
        unlisten();
        return;
      }
      offNav = unlisten;
    });

    return () => {
      cancelled = true;
      offHealth?.();
      offNav?.();
    };
  }, []);

  async function hideAndReset() {
    setActiveTab("translate");
    try {
      await invoke("hide_settings_window");
    } catch {
      // no-op
    }
  }

  async function retryHealthCheck() {
    try {
      const code = await invoke<string>("check_llm_health");
      if (code === "healthy") {
        setHealthWarning(null);
      } else {
        setHealthWarning({
          status: code,
          message: `重試後仍異常: ${code}`,
        });
      }
    } catch (error) {
      setHealthWarning({
        status: "error",
        message: String(error),
      });
    }
  }

  return (
    <div className="flex h-screen min-h-0 flex-col bg-background text-foreground">
      {healthWarning ? (
        <div className="px-4 pt-3">
          <Banner
            tone="warning"
            size="sm"
            title="服務警示"
            description={healthWarning.message}
            action={(
              <Button type="button" variant="secondary" size="sm" onClick={() => void retryHealthCheck()}>
                重新檢查
              </Button>
            )}
          />
        </div>
      ) : null}

      <TabNav
        orientation="vertical"
        value={activeTab}
        onValueChange={(value) => {
          if (isTabKey(value)) {
            setActiveTab(value);
          }
        }}
        className="min-h-0 flex-1 gap-0"
      >
        <aside className="w-32 shrink-0 border-r border-border bg-muted/30 p-3">
          <TabNavList orientation="vertical" className="h-auto w-full bg-transparent p-0">
            {TAB_ITEMS.map((item) => (
              <TabNavTrigger
                key={item.key}
                orientation="vertical"
                value={item.key}
                className="w-full justify-start rounded-md"
              >
                {item.label}
              </TabNavTrigger>
            ))}
          </TabNavList>
        </aside>

        <div className="min-w-0 flex-1 overflow-y-auto p-4">
          <TabNavContent value="translate" className="mt-0">
            <TranslateTab />
          </TabNavContent>
          <TabNavContent value="speech" className="mt-0">
            <SpeechTab />
          </TabNavContent>
          <TabNavContent value="output" className="mt-0">
            <OutputTab />
          </TabNavContent>
          <TabNavContent value="about" className="mt-0">
            <AboutTab />
          </TabNavContent>
        </div>
      </TabNav>

      <footer className="flex items-center justify-end gap-2 border-t border-border bg-muted/20 px-4 py-3">
        <Button type="button" variant="secondary" onClick={() => void hideAndReset()}>
          取消
        </Button>
        <Button type="button" variant="primary" onClick={() => void hideAndReset()}>
          確定
        </Button>
      </footer>
    </div>
  );
}
