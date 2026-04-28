import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type PixtralProgressPhase = "gguf" | "mmproj";

export type PixtralProgress = {
  phase: PixtralProgressPhase;
  downloaded: number;
  total: number;
  percent: number;
};

export type PixtralInstallSnapshot = {
  installing: boolean;
  progress: PixtralProgress | null;
  error: string | null;
  installed: boolean;
};

const snapshot: PixtralInstallSnapshot = {
  installing: false,
  progress: null,
  error: null,
  installed: false,
};

const subscribers = new Set<(value: PixtralInstallSnapshot) => void>();
let listenersReady = false;
let unlistenProgress: UnlistenFn | null = null;
let unlistenDone: UnlistenFn | null = null;
let unlistenFailed: UnlistenFn | null = null;

function emitSnapshot() {
  const value = { ...snapshot };
  for (const cb of subscribers) {
    cb(value);
  }
}

async function ensureListeners() {
  if (listenersReady) {
    return;
  }
  listenersReady = true;

  unlistenProgress = await listen<PixtralProgress>("pixtral-install-progress", (event) => {
    snapshot.installing = true;
    snapshot.error = null;
    snapshot.progress = event.payload;
    emitSnapshot();
  });

  unlistenDone = await listen("pixtral-install-done", () => {
    snapshot.installing = false;
    snapshot.error = null;
    snapshot.installed = true;
    emitSnapshot();
  });

  unlistenFailed = await listen<string>("pixtral-install-failed", (event) => {
    snapshot.installing = false;
    snapshot.error = event.payload;
    emitSnapshot();
  });
}

export async function checkPixtralInstalled(): Promise<boolean> {
  const installed = await invoke<boolean>("check_pixtral_installed");
  snapshot.installed = installed;
  emitSnapshot();
  return installed;
}

export function listAvailableOutputLangs(): Promise<string[]> {
  return invoke("list_available_output_langs");
}

export async function installPixtral(): Promise<void> {
  await ensureListeners();
  snapshot.installing = true;
  snapshot.error = null;
  emitSnapshot();
  try {
    await invoke("install_pixtral");
  } catch (err) {
    snapshot.installing = false;
    snapshot.error = String(err);
    emitSnapshot();
    throw err;
  }
}

export async function subscribePixtralInstall(
  cb: (value: PixtralInstallSnapshot) => void,
): Promise<() => void> {
  await ensureListeners();
  subscribers.add(cb);
  cb({ ...snapshot });
  return () => {
    subscribers.delete(cb);
  };
}

export function getPixtralInstallSnapshot(): PixtralInstallSnapshot {
  return { ...snapshot };
}

export function resetPixtralInstallListenersForTest() {
  if (unlistenProgress) {
    unlistenProgress();
    unlistenProgress = null;
  }
  if (unlistenDone) {
    unlistenDone();
    unlistenDone = null;
  }
  if (unlistenFailed) {
    unlistenFailed();
    unlistenFailed = null;
  }
  listenersReady = false;
}
