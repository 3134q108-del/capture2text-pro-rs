import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { useEffect, useState } from "react";

const VLM_ENDPOINT = "http://localhost:11434";
const MODEL_NAME = "qwen3-vl:4b-instruct";
const UPSTREAM_URL = "https://capture2text.sourceforge.net/";
const FORK_URL = "https://github.com/3134q108-del/capture2text-pro-rs";

function formatVlm(code: string): string {
  if (code === "healthy") return "✓ VLM 正常";
  if (code === "vlm_runtime_down") return "✗ VLM 服務未就緒";
  if (code.startsWith("model_missing:")) {
    return `✗ 模型缺失：${code.slice("model_missing:".length)}`;
  }
  if (code.startsWith("unknown:")) {
    return `✗ 未知錯誤：${code.slice("unknown:".length)}`;
  }
  return code;
}

export default function AboutTab() {
  const [version, setVersion] = useState<string>("...");
  const [vlmStatus, setVlmStatus] = useState<string>("");
  const [updateStatus, setUpdateStatus] = useState<string>("");
  const [exportDir, setExportDir] = useState<string>("");
  const [importDir, setImportDir] = useState<string>("");
  const [statusMsg, setStatusMsg] = useState<string>("");

  useEffect(() => {
    void getVersion()
      .then(setVersion)
      .catch(() => setVersion("unknown"));
  }, []);

  async function checkVlm() {
    setVlmStatus("檢查中…");
    try {
      const result = await invoke<string>("check_vlm_health");
      setVlmStatus(formatVlm(result));
    } catch (err) {
      setVlmStatus(`錯誤：${err}`);
    }
  }

  async function checkUpdate() {
    setUpdateStatus("檢查中…");
    try {
      const tag = await invoke<string>("check_for_updates");
      if (tag === "no_release") {
        setUpdateStatus("尚未發佈正式 release");
        return;
      }
      setUpdateStatus(`最新版本：${tag}（目前 v${version}）`);
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
      setStatusMsg("請輸入匯出路徑");
      return;
    }
    try {
      const result = await invoke<string>("export_settings", {
        targetDir: exportDir,
      });
      setStatusMsg(`✓ ${result}`);
    } catch (err) {
      setStatusMsg(`匯出失敗：${err}`);
    }
  }

  async function doImport() {
    if (!importDir.trim()) {
      setStatusMsg("請輸入匯入路徑");
      return;
    }
    try {
      const result = await invoke<string>("import_settings", {
        sourceDir: importDir,
      });
      setStatusMsg(`✓ ${result}`);
    } catch (err) {
      setStatusMsg(`匯入失敗：${err}`);
    }
  }

  return (
    <div className="settings-translate-root">
      <section className="settings-section">
        <h2>Capture2Text Pro v{version}</h2>
        <p style={{ margin: 0, color: "var(--c2t-text-muted)" }}>
          Windows OCR + 翻譯 + 朗讀（Tauri + Rust 桌面版）
        </p>
      </section>

      <section className="settings-section">
        <h2>OCR + 翻譯模型</h2>
        <div>
          模型：<code>{MODEL_NAME}</code>
        </div>
        <div>
          服務：<code>{VLM_ENDPOINT}</code>（llama.cpp 服務）
        </div>
        <div style={{ marginTop: 6 }}>
          <button className="c2t-btn" onClick={checkVlm}>
            檢查 VLM 服務連線
          </button>
          {vlmStatus && <span style={{ marginLeft: 10 }}>{vlmStatus}</span>}
        </div>
      </section>

      <section className="settings-section">
        <h2>快捷鍵</h2>
        <ul style={{ margin: 0, paddingLeft: 18 }}>
          <li>
            <kbd>Win</kbd>+<kbd>Q</kbd>：截圖辨識 + 翻譯
          </li>
          <li>
            <kbd>Win</kbd>+<kbd>W</kbd>：只擷取原文
          </li>
          <li>
            <kbd>Win</kbd>+<kbd>E</kbd>：重翻譯目前內容
          </li>
        </ul>
      </section>

      <section className="settings-section">
        <h2>語音合成引擎</h2>
        <div>Microsoft Azure TTS：支援多語系 voice 與試聽</div>
      </section>

      <section className="settings-section">
        <h2>授權與來源</h2>
        <div>原始專案：Christopher Brochtrup</div>
        <div>授權：GPL-3.0</div>
        <div style={{ marginTop: 6, display: "flex", gap: 8 }}>
          <button className="c2t-btn" onClick={() => openUrl(UPSTREAM_URL)}>
            原專案網站
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
            檢查最新版本
          </button>
          {updateStatus && <span style={{ marginLeft: 10 }}>{updateStatus}</span>}
        </div>
      </section>

      <section className="settings-section">
        <h2>設定匯出 / 匯入</h2>
        <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          <div>
            <label>
              匯出路徑（會建立 Capture2TextPro-backup/）
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
              匯入來源路徑
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
