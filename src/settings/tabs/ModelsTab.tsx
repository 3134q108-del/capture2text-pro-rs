import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import {
  Button,
  Card,
  CardContent,
  Checkbox,
  ProgressBar,
  Section,
  SectionBody,
  SectionHeader,
  useSnackbar,
} from "@/components/ui";

type ModelInfo = {
  id: string;
  display_name: string;
  size_mb: number;
  supported_lang_codes: string[];
  downloaded: boolean;
  active: boolean;
};

type DownloadProgress = {
  model_id: string;
  file: string;
  downloaded: number;
  total: number;
};

const SPEED_HINT: Record<string, string> = {
  Qwen3Vl2bInstruct: "0.3-0.8 秒/張 (RTX 4070Ti) / 24 秒 (CPU)",
  Qwen3Vl4bInstruct: "0.5-1.5 秒/張 (RTX 4070Ti) / 47 秒 (CPU)",
  Qwen3Vl8bInstruct: "1-3 秒/張 (RTX 4070Ti) / 60-100 秒 (CPU)",
};

const TIER_HINT: Record<string, string> = {
  Qwen3Vl2bInstruct: "輕量檔位，8 種語言：中(繁/簡)、英、日、韓、法、德、西",
  Qwen3Vl4bInstruct: "甜蜜點，14 種語言 (上面 + 葡、義、俄、印、土、波)",
  Qwen3Vl8bInstruct: "品質檔位，全 20 種語言 (上面 + 越、阿、泰、印地、希、希伯來)",
};

export default function ModelsTab() {
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [progress, setProgress] = useState<Record<string, { pct: number; file: string }>>({});
  const [annotatorMode, setAnnotatorMode] = useState(false);
  const snackbar = useSnackbar();

  useEffect(() => {
    void refresh();
  }, []);

  useEffect(() => {
    let off1: undefined | (() => void);
    let off2: undefined | (() => void);
    let off3: undefined | (() => void);

    listen<DownloadProgress>("model-download-progress", (e) => {
      const pct = e.payload.total > 0 ? (e.payload.downloaded / e.payload.total) * 100 : 0;
      setProgress((prev) => ({
        ...prev,
        [e.payload.model_id]: { pct, file: e.payload.file },
      }));
    }).then((fn) => {
      off1 = fn;
    });

    listen<string>("model-download-complete", (e) => {
      setProgress((prev) => {
        const { [e.payload]: _removed, ...rest } = prev;
        return rest;
      });
      void refresh();
      snackbar.show("success", `模型 ${e.payload} 下載完成`);
    }).then((fn) => {
      off2 = fn;
    });

    listen<string>("active-model-changed", () => {
      void refresh();
    }).then((fn) => {
      off3 = fn;
    });

    return () => {
      off1?.();
      off2?.();
      off3?.();
    };
  }, []);

  async function refresh() {
    try {
      const [list, annotator] = await Promise.all([
        invoke<ModelInfo[]>("get_models_list"),
        invoke<boolean>("get_annotator_mode"),
      ]);
      setModels(list);
      setAnnotatorMode(annotator);
    } catch (error) {
      snackbar.show("error", String(error));
    }
  }

  async function downloadModel(id: string) {
    try {
      setProgress((prev) => ({ ...prev, [id]: { pct: 0, file: "gguf" } }));
      await invoke("download_model", { id });
    } catch (error) {
      snackbar.show("error", String(error));
      setProgress((prev) => {
        const { [id]: _removed, ...rest } = prev;
        return rest;
      });
    }
  }

  async function setActive(id: string) {
    try {
      snackbar.show("info", "切換模型中，請稍候 (重啟 llama-server，~10-30 秒)...");
      await invoke("set_active_model", { id });
      snackbar.show("success", "已切換為使用中");
      void refresh();
    } catch (error) {
      snackbar.show("error", "切換失敗: " + String(error));
    }
  }

  async function deleteModel(model: ModelInfo) {
    const confirmed = window.confirm(
      `確定要移除 ${model.display_name}？檔案會從磁碟移除（約 ${model.size_mb} MB）。`,
    );
    if (!confirmed) {
      return;
    }

    try {
      await invoke("delete_model", { id: model.id });
      snackbar.show("success", `已移除 ${model.display_name}`);
    } catch (error) {
      snackbar.show("error", `移除失敗: ${String(error)}`);
    }
  }

  async function changeAnnotator(value: boolean) {
    setAnnotatorMode(value);
    try {
      await invoke("set_annotator_mode", { value });
    } catch (error) {
      snackbar.show("error", String(error));
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <Section>
        <SectionHeader title="模型" description="選擇要使用的本機 AI 模型。已下載的可隨時切換或刪除。" />
        <SectionBody>
          <div className="mb-3 space-y-1 rounded-md border border-border bg-muted/30 p-3 text-xs text-muted-foreground">
            <div>三個檔位由小到大，品質與所需資源成正比。可隨時切換，已下載的不重複下載。</div>
            <div>顯卡 VRAM 不足以容納整個模型時，自動 fallback CPU，速度會明顯變慢。</div>
            <div>
              <strong>VRAM 建議:</strong>
              2B = 6 GB+ ｜ 4B = 8 GB+ ｜ 8B = 12 GB+
            </div>
          </div>
          <div className="grid gap-3">
            {models.map((m) => {
              const dl = progress[m.id];
              const sizeGb = (m.size_mb / 1024).toFixed(1);
              return (
                <Card key={m.id}>
                  <CardContent className="flex flex-col gap-2 p-4">
                    <div className="flex items-center justify-between gap-2">
                      <div className="font-medium text-foreground">
                        {m.display_name}
                        {m.active ? <span className="ml-2 text-xs text-green-700">✅ 使用中</span> : null}
                      </div>
                      <div className="text-xs text-muted-foreground">{sizeGb} GB</div>
                    </div>
                    <div className="text-xs text-muted-foreground">
                      速度: {SPEED_HINT[m.id] ?? ""}
                    </div>
                    <div className="text-xs text-muted-foreground">
                      {TIER_HINT[m.id] ?? ""}
                    </div>

                    {m.id === "Qwen3Vl8bInstruct" ? (
                      <div className="mt-2 flex items-center gap-2">
                        <Checkbox
                          checked={annotatorMode}
                          onCheckedChange={(value) => void changeAnnotator(value === true)}
                          label="註解英文詞 (將英文翻成中文 + 括號夾原文)"
                          description="僅 8B 完整支援。輕量模型可能效果不佳。"
                        />
                      </div>
                    ) : null}

                    {dl ? (
                      <div className="flex items-center gap-2">
                        <ProgressBar value={dl.pct} max={100} className="flex-1" />
                        <span className="text-xs text-muted-foreground">
                          {dl.file} {dl.pct.toFixed(0)}%
                        </span>
                      </div>
                    ) : !m.downloaded ? (
                      <div>
                        <Button type="button" variant="primary" size="sm" onClick={() => void downloadModel(m.id)}>
                          下載
                        </Button>
                      </div>
                    ) : (
                      <div className="flex flex-wrap items-center gap-2">
                        {!m.active ? (
                          <Button type="button" variant="primary" size="sm" onClick={() => void setActive(m.id)}>
                            設為使用中
                          </Button>
                        ) : null}
                        <Button type="button" variant="destructive" size="sm" onClick={() => void deleteModel(m)}>
                          刪除
                        </Button>
                      </div>
                    )}
                  </CardContent>
                </Card>
              );
            })}
          </div>
        </SectionBody>
      </Section>
    </div>
  );
}
