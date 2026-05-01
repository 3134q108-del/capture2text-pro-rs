use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tauri::{
    menu::{CheckMenuItem, IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Listener, Wry,
};

use crate::{
    output_lang,
    window_state::{self, ClipboardMode, WindowState},
};

struct TrayMenuState {
    show_popup_item: CheckMenuItem<Wry>,
    clip_none_item: CheckMenuItem<Wry>,
    clip_original_item: CheckMenuItem<Wry>,
    clip_translated_item: CheckMenuItem<Wry>,
    clip_both_item: CheckMenuItem<Wry>,
    lang_items: Vec<(String, CheckMenuItem<Wry>)>,
    lang_id_to_code: HashMap<String, String>,
    scenario_items: Vec<CheckMenuItem<Wry>>,
    enabled_langs: Vec<String>,
}

type MenuBuild = (Menu<Wry>, TrayMenuState);

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let state = window_state::get();
    let current_lang = output_lang::current();
    let (menu, menu_state) = build_tray_menu(app, &state, &current_lang)?;
    let shared_state = Arc::new(Mutex::new(menu_state));

    let shared_state_for_menu = Arc::clone(&shared_state);
    let builder = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app: &AppHandle, event: MenuEvent| match event.id().as_ref() {
            "show_settings" => {
                let _ = crate::commands::result_window::show_settings_window(app.clone());
            }
            "toggle_show_popup" => {
                if let Ok(guard) = shared_state_for_menu.lock() {
                    let next = !guard.show_popup_item.is_checked().ok().unwrap_or(false);
                    window_state::set_popup_show_enabled(next);
                    let _ = guard.show_popup_item.set_checked(next);
                }
            }
            "clip_none" => {
                window_state::set_clipboard_mode(ClipboardMode::None);
                sync_clipboard_checks(ClipboardMode::None, &shared_state_for_menu);
            }
            "clip_original" => {
                window_state::set_clipboard_mode(ClipboardMode::OriginalOnly);
                sync_clipboard_checks(ClipboardMode::OriginalOnly, &shared_state_for_menu);
            }
            "clip_translated" => {
                window_state::set_clipboard_mode(ClipboardMode::TranslatedOnly);
                sync_clipboard_checks(ClipboardMode::TranslatedOnly, &shared_state_for_menu);
            }
            "clip_both" => {
                window_state::set_clipboard_mode(ClipboardMode::Both);
                sync_clipboard_checks(ClipboardMode::Both, &shared_state_for_menu);
            }
            id if id.starts_with("target_lang_") => {
                if let Ok(guard) = shared_state_for_menu.lock() {
                    if let Some(code) = guard.lang_id_to_code.get(id) {
                        let _ = output_lang::set(code);
                        for (lang_code, item) in &guard.lang_items {
                            let _ = item.set_checked(lang_code == code);
                        }
                    }
                }
            }
            id if id.starts_with("scenario_") => {
                let sid = id.trim_start_matches("scenario_").to_string();
                if crate::scenarios::set_active_scenario_id(sid.clone()).is_ok() {
                    let selected = format!("scenario_{sid}");
                    if let Ok(guard) = shared_state_for_menu.lock() {
                        for item in &guard.scenario_items {
                            let _ = item.set_checked(item.id().as_ref() == selected.as_str());
                        }
                    }
                }
            }
            "show_about" => {
                if crate::commands::result_window::show_settings_window(app.clone()).is_ok() {
                    let _ = app.emit_to("settings", "settings-navigate", "about");
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        });

    let builder = match app.default_window_icon().cloned() {
        Some(icon) => builder.icon(icon),
        None => builder,
    };
    let _tray = builder.build(app)?;

    let shared_state_for_output = Arc::clone(&shared_state);
    app.listen("output-language-changed", move |event| {
        if let Ok(lang) = serde_json::from_str::<String>(event.payload()) {
            if let Ok(guard) = shared_state_for_output.lock() {
                for (code, item) in &guard.lang_items {
                    let _ = item.set_checked(code == &lang);
                }
            }
        }
    });

    let shared_state_for_scenarios = Arc::clone(&shared_state);
    app.listen("scenarios-changed", move |_event| {
        let active = crate::scenarios::get_active_scenario_id();
        let selected = format!("scenario_{active}");
        if let Ok(guard) = shared_state_for_scenarios.lock() {
            for item in &guard.scenario_items {
                let _ = item.set_checked(item.id().as_ref() == selected.as_str());
            }
        }
    });

    let app_for_state = app.clone();
    let shared_state_for_state = Arc::clone(&shared_state);
    app.listen("window-state-changed", move |event| {
        let Ok(next_state) = serde_json::from_str::<WindowState>(event.payload()) else {
            return;
        };

        if let Ok(guard) = shared_state_for_state.lock() {
            let _ = guard.show_popup_item.set_checked(next_state.popup_show_enabled);
            let _ = guard
                .clip_none_item
                .set_checked(next_state.clipboard_mode == ClipboardMode::None);
            let _ = guard
                .clip_original_item
                .set_checked(next_state.clipboard_mode == ClipboardMode::OriginalOnly);
            let _ = guard
                .clip_translated_item
                .set_checked(next_state.clipboard_mode == ClipboardMode::TranslatedOnly);
            let _ = guard
                .clip_both_item
                .set_checked(next_state.clipboard_mode == ClipboardMode::Both);
        }

        let should_rebuild = if let Ok(guard) = shared_state_for_state.lock() {
            guard.enabled_langs != next_state.enabled_langs
        } else {
            false
        };
        if !should_rebuild {
            return;
        }

        let current_lang = output_lang::current();
        let Ok((new_menu, new_state)) = build_tray_menu(&app_for_state, &next_state, &current_lang) else {
            return;
        };

        let app_for_menu = app_for_state.clone();
        let _ = app_for_state.run_on_main_thread(move || {
            if let Some(tray) = app_for_menu.tray_by_id("main") {
                let _ = tray.set_menu(Some(new_menu));
            }
        });

        if let Ok(mut guard) = shared_state_for_state.lock() {
            *guard = new_state;
        }
    });

    Ok(())
}

fn sync_clipboard_checks(mode: ClipboardMode, shared_state: &Arc<Mutex<TrayMenuState>>) {
    if let Ok(guard) = shared_state.lock() {
        let _ = guard.clip_none_item.set_checked(mode == ClipboardMode::None);
        let _ = guard
            .clip_original_item
            .set_checked(mode == ClipboardMode::OriginalOnly);
        let _ = guard
            .clip_translated_item
            .set_checked(mode == ClipboardMode::TranslatedOnly);
        let _ = guard.clip_both_item.set_checked(mode == ClipboardMode::Both);
    }
}

fn build_tray_menu(app: &AppHandle, state: &WindowState, current_lang: &str) -> tauri::Result<MenuBuild> {
    let show_settings = MenuItem::with_id(app, "show_settings", "開啟設定...", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;

    let toggle_show_popup = CheckMenuItem::with_id(
        app,
        "toggle_show_popup",
        "顯示結果視窗",
        true,
        state.popup_show_enabled,
        None::<&str>,
    )?;
    let clip_none = CheckMenuItem::with_id(
        app,
        "clip_none",
        "不寫入剪貼簿",
        true,
        state.clipboard_mode == ClipboardMode::None,
        None::<&str>,
    )?;
    let clip_original = CheckMenuItem::with_id(
        app,
        "clip_original",
        "僅原文",
        true,
        state.clipboard_mode == ClipboardMode::OriginalOnly,
        None::<&str>,
    )?;
    let clip_translated = CheckMenuItem::with_id(
        app,
        "clip_translated",
        "僅譯文",
        true,
        state.clipboard_mode == ClipboardMode::TranslatedOnly,
        None::<&str>,
    )?;
    let clip_both = CheckMenuItem::with_id(
        app,
        "clip_both",
        "原文 + 譯文",
        true,
        state.clipboard_mode == ClipboardMode::Both,
        None::<&str>,
    )?;
    let clip_submenu = Submenu::with_items(
        app,
        "剪貼簿輸出",
        true,
        &[&clip_none, &clip_original, &clip_translated, &clip_both],
    )?;
    let sep2 = PredefinedMenuItem::separator(app)?;

    let (lang_submenu, lang_items, lang_id_to_code) =
        build_lang_submenu(app, &state.enabled_langs, current_lang)?;

    let sep_between_submenus = PredefinedMenuItem::separator(app)?;
    let scenarios = crate::scenarios::list_runtime().unwrap_or_else(|_| crate::scenarios::builtin_default());
    let active_scenario_id = crate::scenarios::get_active_scenario_id();
    let scenario_items: Vec<_> = scenarios
        .iter()
        .map(|scenario| {
            CheckMenuItem::with_id(
                app,
                format!("scenario_{}", scenario.id),
                scenario.name.clone(),
                true,
                scenario.id == active_scenario_id,
                None::<&str>,
            )
        })
        .collect::<Result<_, _>>()?;
    let scenario_refs: Vec<&dyn IsMenuItem<_>> = scenario_items
        .iter()
        .map(|item| item as &dyn IsMenuItem<_>)
        .collect();
    let scenario_submenu = Submenu::with_items(app, "情境", true, &scenario_refs)?;

    let sep3 = PredefinedMenuItem::separator(app)?;
    let show_about = MenuItem::with_id(app, "show_about", "關於...", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &show_settings,
            &sep1,
            &toggle_show_popup,
            &clip_submenu,
            &sep2,
            &lang_submenu,
            &sep_between_submenus,
            &scenario_submenu,
            &sep3,
            &show_about,
            &quit,
        ],
    )?;

    Ok((
        menu,
        TrayMenuState {
            show_popup_item: toggle_show_popup,
            clip_none_item: clip_none,
            clip_original_item: clip_original,
            clip_translated_item: clip_translated,
            clip_both_item: clip_both,
            lang_items,
            lang_id_to_code,
            scenario_items,
            enabled_langs: state.enabled_langs.clone(),
        },
    ))
}

fn build_lang_submenu(
    app: &AppHandle,
    enabled_langs: &[String],
    current_lang: &str,
) -> tauri::Result<(Submenu<Wry>, Vec<(String, CheckMenuItem<Wry>)>, HashMap<String, String>)> {
    let mut lang_items = Vec::new();
    let mut lang_id_to_code = HashMap::new();

    for code in enabled_langs {
        let Some(lang) = crate::languages::by_code(code) else {
            continue;
        };
        let menu_id = format!("target_lang_{}", code.replace('-', "_").to_ascii_lowercase());
        let item = CheckMenuItem::with_id(
            app,
            &menu_id,
            format!("{} ({})", lang.native_name, lang.code.as_str()),
            true,
            current_lang == code,
            None::<&str>,
        )?;
        lang_id_to_code.insert(menu_id, code.clone());
        lang_items.push((code.clone(), item));
    }

    if lang_items.is_empty() {
        let fallback = CheckMenuItem::with_id(
            app,
            "target_lang_zh_tw",
            "繁體中文 (zh-TW)",
            true,
            current_lang == "zh-TW",
            None::<&str>,
        )?;
        lang_id_to_code.insert("target_lang_zh_tw".to_string(), "zh-TW".to_string());
        lang_items.push(("zh-TW".to_string(), fallback));
    }

    let refs: Vec<&dyn IsMenuItem<_>> = lang_items
        .iter()
        .map(|(_, item)| item as &dyn IsMenuItem<_>)
        .collect();
    let submenu = Submenu::with_items(app, "目標語言", true, &refs)?;
    Ok((submenu, lang_items, lang_id_to_code))
}
