import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { useEffect, useMemo, useState } from "react";
import {
  checkPixtralInstalled,
  getPixtralInstallSnapshot,
  installPixtral,
  subscribePixtralInstall,
  type PixtralInstallSnapshot,
} from "../../services/llama";

const VLM_ENDPOINT = "http://localhost:11434";
const MODEL_NAME = "qwen3-vl:4b-instruct";
const UPSTREAM_URL = "https://capture2text.sourceforge.net/";
const FORK_URL = "https://github.com/3134q108-del/capture2text-pro-rs";

function formatVlm(code: string): string {
  if (code === "healthy") return "VLM 服務正常";
  if (code === "vlm_runtime_down") return "VLM 服務未就緒";
  if (code.startsWith("model_missing:")) {
    return `模型遺失：${code.slice("model_missing:".length)}`;
  }
  if (code.startsWith("unknown:")) {
    return `未知錯誤：${code.slice("unknown:".length)}`;
  }
  return code;
}

function formatBytes(n: number): string {
  if (n <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = n;
  let index = 0;
  while (value >= 1024 && index < units.length - 1) {
    value /= 1024;
    index += 1;
  }
  return `${value.toFixed(index === 0 ? 0 : 1)} ${units[index]}`;
}

export default function AboutTab() {
  const [version, setVersion] = useState("...");
  const [vlmStatus, setVlmStatus] = useState("");
  const [updateStatus, setUpdateStatus] = useState("");
  const [exportDir, setExportDir] = useState("");
  const [importDir, setImportDir] = useState("");
  const [statusMsg, setStatusMsg] = useState("");
  const [pixtral, setPixtral] = useState<PixtralInstallSnapshot>(getPixtralInstallSnapshot());

  useEffect(() => {
    void getVersion()
      .then(setVersion)
      .catch(() => setVersion("unknown"));
  }, []);

  useEffect(() => {
    let disposed = false;
    let off: null | (() => void) = null;

    const setup = async () => {
      off = await subscribePixtralInstall((next) => {
        setPixtral(next);
      });
      if (disposed) {
        off();
        off = null;
      }
    };

    void setup();
    void checkPixtralInstalled().catch(() => {});
    return () => {
      disposed = true;
      off?.();
    };
  }, []);

  const progressPercent = useMemo(() => {
    if (!pixtral.progress) return 0;
    return Math.max(0, Math.min(100, pixtral.progress.percent));
  }, [pixtral.progress]);

  async function checkVlm() {
    setVlmStatus("檢查中...");
    try {
      const result = await invoke<string>("check_vlm_health");
      setVlmStatus(formatVlm(result));
    } catch (err) {
      setVlmStatus(`錯誤：${err}`);
    }
  }

  async function checkUpdate() {
    setUpdateStatus("檢查中...");
    try {
      const tag = await invoke<string>("check_for_updates");
      if (tag === "no_release") {
        setUpdateStatus("目前找不到 release");
        return;
      }
      setUpdateStatus(`有新版本 ${tag}（目前 v${version}）`);
    } catch (err) {
      setUpdateStatus(`檢查失敗：${err}`);
    }
  }

  async function openUrl(url: string) {
    try {
      await invoke("open_external_url", { url });
    } catch (err) {
      setStatusMsg(String(err));
    }
  }

  async function doExport() {
    if (!exportDir.trim()) {
      setStatusMsg("請輸入匯出資料夾");
      return;
    }
    try {
      const result = await invoke<string>("export_settings", {
        targetDir: exportDir,
      });
      setStatusMsg(result);
    } catch (err) {
      setStatusMsg(`匯出失敗：${err}`);
    }
  }

  async function doImport() {
    if (!importDir.trim()) {
      setStatusMsg("請輸入匯入資料夾");
      return;
    }
    try {
      const result = await invoke<string>("import_settings", {
        sourceDir: importDir,
      });
      setStatusMsg(result);
    } catch (err) {
      setStatusMsg(`匯入失敗：${err}`);
    }
  }

  async function handleInstallPixtral() {
    try {
      await installPixtral();
    } catch (err) {
      setStatusMsg(String(err));
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
        <div style={{ marginTop: 6 }}>
          <button className="c2t-btn" onClick={checkVlm}>
            檢查 VLM 服務連線
          </button>
          {vlmStatus && <span style={{ marginLeft: 10 }}>{vlmStatus}</span>}
        </div>
      </section>

      <section className="settings-section">
        <h2>擴充語言模組（Pixtral）</h2>
        <div className="pixtral-card">
          <div className="pixtral-card-row">
            <span>狀態</span>
            <span>{pixtral.installed ? "已安裝" : "未安裝"}</span>
          </div>
          <div className="pixtral-card-row">
            <span>對應語言</span>
            <span>de-DE / fr-FR</span>
          </div>
          <div className="pixtral-card-row">
            <span>下載內容</span>
            <span>GGUF + mmproj（約 7.5GB）</span>
          </div>
          <div className="pixtral-actions">
            <button
              className="c2t-btn c2t-btn-primary"
              onClick={handleInstallPixtral}
              disabled={pixtral.installing || pixtral.installed}
            >
              {pixtral.installing ? "安裝中..." : pixtral.installed ? "已安裝" : "安裝 Pixtral 模組"}
            </button>
          </div>

          {pixtral.progress && (
            <div className="pixtral-progress">
              <div className="pixtral-progress-header">
                <span>階段：{pixtral.progress.phase === "gguf" ? "GGUF" : "mmproj"}</span>
                {pixtral.progress.total > 0 ? (
                  <span>{progressPercent.toFixed(1)}%</span>
                ) : (
                  <span>下載中...</span>
                )}
              </div>
              {pixtral.progress.total > 0 ? (
                <>
                  <div className="pixtral-progress-bar">
                    <div
                      className="pixtral-progress-fill"
                      style={{ width: `${progressPercent}%` }}
                    />
                  </div>
                  <div className="pixtral-progress-meta">
                    {formatBytes(pixtral.progress.downloaded)} / {formatBytes(pixtral.progress.total)}
                  </div>
                </>
              ) : (
                <div className="pixtral-progress-meta">
                  下載中，已下載 {formatBytes(pixtral.progress.downloaded)}
                </div>
              )}
            </div>
          )}

          {pixtral.error && <div className="pixtral-error">安裝失敗：{pixtral.error}</div>}
        </div>
      </section>

      <section className="settings-section">
        <h2>專案資訊</h2>
        <div style={{ marginTop: 6, display: "flex", gap: 8 }}>
          <button className="c2t-btn" onClick={() => openUrl(UPSTREAM_URL)}>
            原始專案網站
          </button>
          <button className="c2t-btn" onClick={() => openUrl(FORK_URL)}>
            Fork GitHub
          </button>
        </div>
      </section>

      <section className="settings-section">
        <h2>更新檢查</h2>
        <div>
          <button className="c2t-btn" onClick={checkUpdate}>
            檢查新版本
          </button>
          {updateStatus && <span style={{ marginLeft: 10 }}>{updateStatus}</span>}
        </div>
      </section>

      <section className="settings-section">
        <h2>設定匯出 / 匯入</h2>
        <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          <div>
            <label>
              匯出資料夾：
              <input
                type="text"
                value={exportDir}
                onChange={(event) => setExportDir(event.target.value)}
                placeholder="例如 D:\\backup"
              />
            </label>
            <button className="c2t-btn" style={{ marginTop: 6 }} onClick={doExport}>
              匯出設定
            </button>
          </div>
          <div>
            <label>
              匯入資料夾：
              <input
                type="text"
                value={importDir}
                onChange={(event) => setImportDir(event.target.value)}
                placeholder="例如 D:\\backup\\Capture2TextPro-backup"
              />
            </label>
            <button className="c2t-btn" style={{ marginTop: 6 }} onClick={doImport}>
              匯入設定
            </button>
          </div>
        </div>
      </section>

      {statusMsg && <div className="settings-status">{statusMsg}</div>}
    </div>
  );
}
