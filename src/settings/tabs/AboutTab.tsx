import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { useEffect, useState } from "react";

const OLLAMA_ENDPOINT = "http://localhost:11434";
const MODEL_NAME = "qwen3-vl:8b-instruct";
const UPSTREAM_URL = "https://capture2text.sourceforge.net/";
const FORK_URL = "https://github.com/3134q108-del/capture2text-pro-rs";

function formatOllama(code: string): string {
  if (code === "healthy") return "✓ Ollama 正常";
  if (code === "daemon_down") return "✗ Ollama daemon 未啟動";
  if (code.startsWith("model_missing:")) {
    return `✗ 模型未安裝：${code.slice("model_missing:".length)}`;
  }
  if (code.startsWith("unknown:")) {
    return `⚠ 狀態不明：${code.slice("unknown:".length)}`;
  }
  return code;
}

export default function AboutTab() {
  const [version, setVersion] = useState<string>("…");
  const [ollamaStatus, setOllamaStatus] = useState<string>("");
  const [updateStatus, setUpdateStatus] = useState<string>("");
  const [exportDir, setExportDir] = useState<string>("");
  const [importDir, setImportDir] = useState<string>("");
  const [statusMsg, setStatusMsg] = useState<string>("");

  useEffect(() => {
    void getVersion()
      .then(setVersion)
      .catch(() => setVersion("unknown"));
  }, []);

  async function checkOllama() {
    setOllamaStatus("檢查中…");
    try {
      const result = await invoke<string>("check_llm_health");
      setOllamaStatus(formatOllama(result));
    } catch (err) {
      setOllamaStatus(`錯誤：${err}`);
    }
  }

  async function checkUpdate() {
    setUpdateStatus("查詢中…");
    try {
      const tag = await invoke<string>("check_for_updates");
      if (tag === "no_release") {
        setUpdateStatus("尚未發佈正式版");
        return;
      }
      setUpdateStatus(`最新版本：${tag}（當前：v${version}）`);
    } catch (err) {
      setUpdateStatus(`查詢失敗：${err}`);
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
      setStatusMsg("請先輸入匯出目錄");
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
      setStatusMsg("請先輸入來源目錄");
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
          Windows OCR + 翻譯 + 朗讀工具（Tauri + Rust 重寫版）
        </p>
      </section>

      <section className="settings-section">
        <h2>OCR + 翻譯模型</h2>
        <div>
          模型：<code>{MODEL_NAME}</code>
        </div>
        <div>
          後端：<code>{OLLAMA_ENDPOINT}</code>
        </div>
        <div style={{ marginTop: 6 }}>
          <button className="c2t-btn" onClick={checkOllama}>
            檢查 Ollama 連線
          </button>
          {ollamaStatus && <span style={{ marginLeft: 10 }}>{ollamaStatus}</span>}
        </div>
      </section>

      <section className="settings-section">
        <h2>快捷鍵</h2>
        <ul style={{ margin: 0, paddingLeft: 18 }}>
          <li>
            <kbd>Win</kbd>+<kbd>Q</kbd>：框選區域擷取
          </li>
          <li>
            <kbd>Win</kbd>+<kbd>W</kbd>：目前視窗擷取
          </li>
          <li>
            <kbd>Win</kbd>+<kbd>E</kbd>：全螢幕擷取
          </li>
        </ul>
      </section>

      <section className="settings-section">
        <h2>語音引擎</h2>
        <div>Microsoft Edge TTS（雲端，免費，需網路）</div>
      </section>

      <section className="settings-section">
        <h2>原版與授權</h2>
        <div>原作者：Christopher Brochtrup</div>
        <div>授權：GPL-3.0</div>
        <div style={{ marginTop: 6, display: "flex", gap: 8 }}>
          <button className="c2t-btn" onClick={() => openUrl(UPSTREAM_URL)}>
            原版官網
          </button>
          <button className="c2t-btn" onClick={() => openUrl(FORK_URL)}>
            本專案 GitHub
          </button>
        </div>
      </section>

      <section className="settings-section">
        <h2>檢查更新</h2>
        <div>
          <button className="c2t-btn" onClick={checkUpdate}>
            立即查詢最新版
          </button>
          {updateStatus && <span style={{ marginLeft: 10 }}>{updateStatus}</span>}
        </div>
      </section>

      <section className="settings-section">
        <h2>設定匯出 / 匯入</h2>
        <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          <div>
            <label>
              匯出目錄（系統會在此建立 Capture2TextPro-backup/ 子目錄）
              <input
                type="text"
                value={exportDir}
                onChange={(event) => setExportDir(event.target.value)}
                placeholder="例：D:\\backup"
              />
            </label>
            <button className="c2t-btn" style={{ marginTop: 6 }} onClick={doExport}>
              匯出設定
            </button>
          </div>
          <div>
            <label>
              匯入來源目錄
              <input
                type="text"
                value={importDir}
                onChange={(event) => setImportDir(event.target.value)}
                placeholder="例：D:\\backup\\Capture2TextPro-backup"
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
