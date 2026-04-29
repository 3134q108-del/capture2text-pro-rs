import { invoke } from "@tauri-apps/api/core";

export type HotkeyModifiers = {
  ctrl: boolean;
  shift: boolean;
  alt: boolean;
  win: boolean;
};

export type HotkeyBinding = {
  modifiers: HotkeyModifiers;
  key_code: number;
};

export type HotkeyConfig = {
  hotkey_q: HotkeyBinding;
  hotkey_w: HotkeyBinding;
  hotkey_e: HotkeyBinding;
};

export function getHotkeyConfig(): Promise<HotkeyConfig> {
  return invoke("get_hotkey_config");
}

export function setHotkeyConfig(config: HotkeyConfig): Promise<void> {
  return invoke("set_hotkey_config", { config });
}

export function resetHotkeyDefault(): Promise<HotkeyConfig> {
  return invoke("reset_hotkey_default");
}
