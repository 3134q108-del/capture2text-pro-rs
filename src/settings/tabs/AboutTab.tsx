import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import {
  Button,
  PathPicker,
  Section,
  SectionBody,
  SectionHeader,
  StatusText,
} from "@/components/ui";

const VLM_ENDPOINT = "http://localhost:11434";
const MODEL_NAME = "qwen3-vl:8b-instruct";

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

export default function AboutTab() {
  const [version, setVersion] = useState("...");
  const [vlmStatus, setVlmStatus] = useState("");
  const [updateStatus, setUpdateStatus] = useState("");
  const [statusMsg, setStatusMsg] = useState("");
  const [exportPath, setExportPath] = useState("");
  const [importPath, setImportPath] = useState("");

  useEffect(() => {
    void getVersion()
      .then(setVersion)
      .catch(() => setVersion("unknown"));
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

  async function runExport(path: string) {
    setExportPath(path);
    try {
      const result = await invoke<string>("export_settings", {
        targetDir: path,
      });
      setStatusMsg(result);
    } catch (error) {
      setStatusMsg(`匯出失敗: ${String(error)}`);
    }
  }

  async function runImport(path: string) {
    setImportPath(path);
    try {
      const result = await invoke<string>("import_settings", {
        sourceDir: path,
      });
      setStatusMsg(result);
    } catch (error) {
      setStatusMsg(`匯入失敗: ${String(error)}`);
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <Section>
        <SectionHeader title={`Capture2Text Pro v${version}`} />
      </Section>

      <Section>
        <SectionHeader title="OCR 與翻譯模型" />
        <SectionBody>
          <StatusText tone="info" size="sm">
            模型: {MODEL_NAME}
          </StatusText>
          <StatusText tone="info" size="sm">
            服務: {VLM_ENDPOINT}（llama.cpp）
          </StatusText>
          <div className="flex flex-wrap items-center gap-2">
            <Button type="button" variant="secondary" onClick={() => void checkVlm()}>
              檢查 VLM 服務連線
            </Button>
            {vlmStatus ? <StatusText tone="info" size="sm">{vlmStatus}</StatusText> : null}
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
          <PathPicker
            mode="directory"
            label="匯出設定"
            value={exportPath}
            placeholder="選擇匯出資料夾"
            buttonLabel="選擇資料夾並匯出"
            onChange={(path) => {
              void runExport(path);
            }}
            onPickError={(message) => setStatusMsg(message)}
          />
          <PathPicker
            mode="directory"
            label="匯入設定"
            value={importPath}
            placeholder="選擇匯入資料夾"
            buttonLabel="選擇資料夾並匯入"
            onChange={(path) => {
              void runImport(path);
            }}
            onPickError={(message) => setStatusMsg(message)}
          />
        </SectionBody>
      </Section>

      {statusMsg ? (
        <StatusText tone="info" size="sm">
          {statusMsg}
        </StatusText>
      ) : null}
    </div>
  );
}
