import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import {
  Button,
  Card,
  CardContent,
  Section,
  SectionBody,
  SectionHeader,
  Select,
  SelectContent as ModelPickerContent,
  SelectItem,
  SelectTrigger as ModelPickerTrigger,
  SelectValue,
  StatusText,
  useSnackbar,
} from "@/components/ui";

const VLM_ENDPOINT = "http://localhost:11500";

type ModelInfo = {
  id: string;
  display_name: string;
  size_mb: number;
  supported_lang_codes: string[];
  downloaded: boolean;
  active: boolean;
};

type ModelStatusView = {
  label: string;
  tone: "info" | "success";
  className?: string;
};

const MODEL_EVENTS = ["model-deleted", "model-download-complete", "active-model-changed"] as const;

function formatVlm(code: string): string {
  if (code === "healthy") return "VLM 服務正常";
  if (code === "vlm_runtime_down") return "VLM 服務未就緒";
  if (code.startsWith("model_missing:")) {
    return `模型缺失: ${code.slice("model_missing:".length)}`;
  }
  if (code.startsWith("unknown:")) {
    return `未知狀態: ${code.slice("unknown:".length)}`;
  }
  return code;
}

function viewModelStatus(model: ModelInfo): ModelStatusView {
  if (model.active) {
    return { label: "使用中", tone: "success" };
  }
  if (model.downloaded) {
    return { label: "已下載", tone: "info", className: "text-primary" };
  }
  return { label: "未下載", tone: "info", className: "text-muted-foreground" };
}

export default function AboutTab() {
  const [version, setVersion] = useState("...");
  const [vlmStatus, setVlmStatus] = useState("");
  const [updateStatus, setUpdateStatus] = useState("");
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [modelsError, setModelsError] = useState("");
  const [deletingModelId, setDeletingModelId] = useState("");
  const snackbar = useSnackbar();

  useEffect(() => {
    void getVersion()
      .then(setVersion)
      .catch(() => setVersion("unknown"));
  }, []);

  useEffect(() => {
    let cleanup: Array<() => void> = [];
    let cancelled = false;

    async function refreshModels() {
      try {
        const list = await invoke<ModelInfo[]>("get_models_list");
        setModels(list);
        setModelsError("");
      } catch (error) {
        setModelsError(`模型清單載入失敗: ${String(error)}`);
        console.error("模型清單載入失敗", error);
      }
    }

    async function setupListeners() {
      const listeners = await Promise.all(
        MODEL_EVENTS.map((eventName) =>
          listen<string>(eventName, () => {
            void refreshModels();
          }),
        ),
      );

      if (cancelled) {
        listeners.forEach((off) => off());
        return;
      }

      cleanup = listeners;
    }

    void refreshModels();
    void setupListeners().catch((error) => {
      setModelsError(`模型事件監聽失敗: ${String(error)}`);
      console.error("模型事件監聽失敗", error);
    });

    return () => {
      cancelled = true;
      cleanup.forEach((off) => off());
    };
  }, []);

  async function checkVlm() {
    setVlmStatus("檢查中...");
    try {
      const result = await invoke<string>("check_vlm_health");
      setVlmStatus(formatVlm(result));
    } catch (error) {
      setVlmStatus(`失敗: ${String(error)}`);
    }
  }

  async function checkUpdate() {
    setUpdateStatus("檢查中...");
    try {
      const tag = await invoke<string>("check_for_updates");
      if (tag === "no_release") {
        setUpdateStatus("目前沒有可用 release");
        return;
      }
      setUpdateStatus(`發現新版本 ${tag}，目前版本 v${version}`);
    } catch (error) {
      setUpdateStatus(`檢查失敗: ${String(error)}`);
    }
  }

  async function exportSettings() {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (!selected || Array.isArray(selected)) {
        return;
      }
      const result = await invoke<string>("export_settings", { targetDir: selected });
      snackbar.show("success", result);
    } catch (error) {
      snackbar.show("error", `匯出失敗: ${String(error)}`);
    }
  }

  async function importSettings() {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (!selected || Array.isArray(selected)) {
        return;
      }
      const result = await invoke<string>("import_settings", { sourceDir: selected });
      snackbar.show("success", result);
    } catch (error) {
      snackbar.show("error", `匯入失敗: ${String(error)}`);
    }
  }

  const removeSelectedModel = async function deleteSelectedModel() {
    if (!deletingModelId) {
      return;
    }
    const model = models.find((item) => item.id === deletingModelId);
    if (!model) {
      return;
    }

    const confirmed = window.confirm(
      `確定要移除 ${model.display_name}？檔案會從磁碟移除（約 ${model.size_mb} MB）。`,
    );
    if (!confirmed) {
      return;
    }

    try {
      await invoke("delete_model", { id: deletingModelId });
      snackbar.show("success", `已移除 ${model.display_name}`);
      setDeletingModelId("");
    } catch (error) {
      snackbar.show("error", `移除失敗: ${String(error)}`);
    }
  };

  const downloadedModels = models.filter((item) => item.downloaded);

  return (
    <div className="flex flex-col gap-4">
      <Section>
        <SectionHeader title={`Capture2Text Pro v${version}`} />
      </Section>

      <Section>
        <SectionHeader title="OCR 與翻譯模型" />
        <SectionBody>
          {modelsError ? (
            <StatusText tone="error" size="sm">
              {modelsError}
            </StatusText>
          ) : null}
          {models.length > 0 ? (
            <div className="grid gap-3">
              {models.map((model) => {
                const status = viewModelStatus(model);

                return (
                  <Card key={model.id}>
                    <CardContent className="flex flex-col gap-2 p-4">
                      <div className="flex flex-wrap items-center justify-between gap-2">
                        <div className="font-medium text-foreground">{model.display_name}</div>
                        <StatusText
                          tone={status.tone}
                          size="sm"
                          className={`shrink-0 rounded-md border border-border bg-muted/30 px-2 py-1 font-medium ${status.className ?? ""}`}
                        >
                          {status.label}
                        </StatusText>
                      </div>
                      <StatusText tone="info" size="sm">
                        {model.size_mb} MB
                      </StatusText>
                    </CardContent>
                  </Card>
                );
              })}
            </div>
          ) : modelsError ? null : (
            <StatusText tone="info" size="sm">
              模型清單載入中...
            </StatusText>
          )}
          <StatusText tone="info" size="sm">
            服務: {VLM_ENDPOINT}（llama.cpp）
          </StatusText>
          <div className="flex flex-wrap items-center gap-2">
            <Button type="button" variant="secondary" onClick={() => void checkVlm()}>
              檢查 VLM 服務連線
            </Button>
            {vlmStatus ? <StatusText tone="info" size="sm">{vlmStatus}</StatusText> : null}
            {downloadedModels.length > 0 ? (
              <div className="ml-auto flex items-center gap-2">
                <Select value={deletingModelId} onValueChange={setDeletingModelId}>
                  <ModelPickerTrigger className="w-48">
                    <SelectValue placeholder="選擇要刪除的模型" />
                  </ModelPickerTrigger>
                  <ModelPickerContent>
                    {downloadedModels.map((item) => (
                      <SelectItem key={item.id} value={item.id}>
                        {item.display_name} ({item.size_mb} MB)
                      </SelectItem>
                    ))}
                  </ModelPickerContent>
                </Select>
                <Button
                  type="button"
                  variant="destructive"
                  size="sm"
                  disabled={!deletingModelId}
                  onClick={() => void removeSelectedModel()}
                >
                  刪除
                </Button>
              </div>
            ) : null}
          </div>
        </SectionBody>
      </Section>

      <Section>
        <SectionHeader title="更新檢查" />
        <SectionBody>
          <div className="flex flex-wrap items-center gap-2">
            <Button type="button" variant="secondary" onClick={() => void checkUpdate()}>
              檢查更新
            </Button>
            {updateStatus ? <StatusText tone="info" size="sm">{updateStatus}</StatusText> : null}
          </div>
        </SectionBody>
      </Section>

      <Section>
        <SectionHeader title="設定匯出與匯入" />
        <SectionBody>
          <div className="flex flex-wrap items-center gap-2">
            <Button type="button" variant="secondary" onClick={() => void exportSettings()}>
              匯出設定
            </Button>
            <Button type="button" variant="secondary" onClick={() => void importSettings()}>
              匯入設定
            </Button>
          </div>
        </SectionBody>
      </Section>
    </div>
  );
}
