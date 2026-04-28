import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";

const VLM_ENDPOINT = "http://localhost:11434";
const MODEL_NAME = "qwen3-vl:8b-instruct";

function formatVlm(code: string): string {
  if (code === "healthy") return "VLM 服務正常";
  if (code === "vlm_runtime_down") return "VLM 服務未就緒";
  if (code.startsWith("model_missing:")) {
    return `模型缺失：${code.slice("model_missing:".length)}`;
  }
  if (code.startsWith("unknown:")) {
    return `未知狀態：${code.slice("unknown:".length)}`;
  }
  return code;
}

export default function AboutTab() {
  const [version, setVersion] = useState("...");
  const [vlmStatus, setVlmStatus] = useState("");
  const [updateStatus, setUpdateStatus] = useState("");
  const [statusMsg, setStatusMsg] = useState("");

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
    } catch (err) {
      setVlmStatus(`錯誤：${String(err)}`);
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
      setUpdateStatus(`可用版本 ${tag}（目前 v${version}）`);
    } catch (err) {
      setUpdateStatus(`檢查失敗：${String(err)}`);
    }
  }

  async function doExport() {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
      });
      if (!selected || Array.isArray(selected)) {
        return;
      }
      const result = await invoke<string>("export_settings", {
        targetDir: selected,
      });
      setStatusMsg(result);
    } catch (err) {
      setStatusMsg(`匯出失敗：${String(err)}`);
    }
  }

  async function doImport() {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
      });
      if (!selected || Array.isArray(selected)) {
        return;
      }
      const result = await invoke<string>("import_settings", {
        sourceDir: selected,
      });
      setStatusMsg(result);
    } catch (err) {
      setStatusMsg(`匯入失敗：${String(err)}`);
    }
  }

  return (
    <div className="settings-translate-root">
      <section className="settings-section">
        <h2>Capture2Text Pro v{version}</h2>
      </section>

      <section className="settings-section">
        <h2>OCR + 翻譯模型</h2>
        <div>模型：{MODEL_NAME}</div>
        <div>服務：{VLM_ENDPOINT}（llama.cpp）</div>
        <div className="settings-inline-actions">
          <button className="c2t-btn" type="button" onClick={() => void checkVlm()}>
            檢查 VLM 服務連線
          </button>
          {vlmStatus && <span className="settings-inline-status">{vlmStatus}</span>}
        </div>
      </section>

      <section className="settings-section">
        <h2>更新檢查</h2>
        <div className="settings-inline-actions">
          <button className="c2t-btn" type="button" onClick={() => void checkUpdate()}>
            檢查更新
          </button>
          {updateStatus && <span className="settings-inline-status">{updateStatus}</span>}
        </div>
      </section>

      <section className="settings-section">
        <h2>設定匯出 / 匯入</h2>
        <div className="settings-editor-actions">
          <button className="c2t-btn" type="button" onClick={() => void doExport()}>
            匯出設定
          </button>
          <button className="c2t-btn" type="button" onClick={() => void doImport()}>
            匯入設定
          </button>
        </div>
      </section>

      {statusMsg && <div className="settings-status">{statusMsg}</div>}
    </div>
  );
}
