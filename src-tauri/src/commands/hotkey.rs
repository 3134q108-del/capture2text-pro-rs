use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct HotkeyBindingPayload {
    pub modifiers: HotkeyModifiersPayload,
    pub key_code: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct HotkeyModifiersPayload {
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub win: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct HotkeyConfigPayload {
    pub hotkey_q: HotkeyBindingPayload,
    pub hotkey_w: HotkeyBindingPayload,
    pub hotkey_e: HotkeyBindingPayload,
}

#[tauri::command]
pub fn get_hotkey_config() -> HotkeyConfigPayload {
    HotkeyConfigPayload {
        hotkey_q: from_state_binding(crate::window_state::hotkey_q()),
        hotkey_w: from_state_binding(crate::window_state::hotkey_w()),
        hotkey_e: from_state_binding(crate::window_state::hotkey_e()),
    }
}

#[tauri::command]
pub fn set_hotkey_config(config: HotkeyConfigPayload) -> Result<(), String> {
    validate_binding(config.hotkey_q)?;
    validate_binding(config.hotkey_w)?;
    validate_binding(config.hotkey_e)?;

    crate::window_state::set_hotkey_q(to_state_binding(config.hotkey_q));
    crate::window_state::set_hotkey_w(to_state_binding(config.hotkey_w));
    crate::window_state::set_hotkey_e(to_state_binding(config.hotkey_e));
    crate::hotkey::reload_from_state();
    Ok(())
}

#[tauri::command]
pub fn reset_hotkey_default() -> Result<HotkeyConfigPayload, String> {
    let defaults = crate::hotkey::keyboard_hook::default_config();
    crate::window_state::set_hotkey_q(defaults.q);
    crate::window_state::set_hotkey_w(defaults.w);
    crate::window_state::set_hotkey_e(defaults.e);
    crate::hotkey::reload_from_state();
    Ok(get_hotkey_config())
}

fn to_state_binding(payload: HotkeyBindingPayload) -> crate::window_state::HotkeyBinding {
    crate::window_state::HotkeyBinding {
        modifiers: crate::window_state::HotkeyModifiers {
            ctrl: payload.modifiers.ctrl,
            shift: payload.modifiers.shift,
            alt: payload.modifiers.alt,
            win: payload.modifiers.win,
        },
        key_code: payload.key_code,
    }
}

fn from_state_binding(binding: crate::window_state::HotkeyBinding) -> HotkeyBindingPayload {
    HotkeyBindingPayload {
        modifiers: HotkeyModifiersPayload {
            ctrl: binding.modifiers.ctrl,
            shift: binding.modifiers.shift,
            alt: binding.modifiers.alt,
            win: binding.modifiers.win,
        },
        key_code: binding.key_code,
    }
}

fn validate_binding(binding: HotkeyBindingPayload) -> Result<(), String> {
    if binding.key_code == 0 {
        return Err("key_code is required".to_string());
    }
    Ok(())
}
