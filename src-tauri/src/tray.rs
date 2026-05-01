use std::collections::HashMap;

use tauri::{
    menu::{CheckMenuItem, IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Listener,
};

use crate::{
    output_lang,
    window_state::{self, ClipboardMode, WindowState},
};

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let current_lang = output_lang::current();
    let state = window_state::get();

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

    let enabled_langs = state.enabled_langs.clone();
    let mut lang_items = Vec::new();
    let mut lang_id_to_code: HashMap<String, String> = HashMap::new();
    for code in enabled_langs {
        let Some(lang) = crate::languages::by_code(&code) else {
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
        lang_items.push((code, item));
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
    let lang_refs: Vec<&dyn IsMenuItem<_>> = lang_items
        .iter()
        .map(|(_, item)| item as &dyn IsMenuItem<_>)
        .collect();
    let lang_submenu = Submenu::with_items(app, "目標語言", true, &lang_refs)?;

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

    let show_popup_item = toggle_show_popup.clone();
    let clip_none_item = clip_none.clone();
    let clip_original_item = clip_original.clone();
    let clip_translated_item = clip_translated.clone();
    let clip_both_item = clip_both.clone();
    let lang_items_for_menu = lang_items
        .iter()
        .map(|(code, item)| (code.clone(), item.clone()))
        .collect::<Vec<_>>();
    let lang_id_to_code_for_menu = lang_id_to_code.clone();
    let scenario_items_for_menu = scenario_items.clone();

    let show_popup_for_menu = show_popup_item.clone();
    let clip_none_for_menu = clip_none_item.clone();
    let clip_original_for_menu = clip_original_item.clone();
    let clip_translated_for_menu = clip_translated_item.clone();
    let clip_both_for_menu = clip_both_item.clone();

    let icon = app.default_window_icon().cloned();
    let builder = TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app: &AppHandle, event: MenuEvent| match event.id().as_ref() {
            "show_settings" => {
                let _ = crate::commands::result_window::show_settings_window(app.clone());
            }
            "toggle_show_popup" => {
                let next = !show_popup_for_menu.is_checked().ok().unwrap_or(false);
                window_state::set_popup_show_enabled(next);
                let _ = show_popup_for_menu.set_checked(next);
            }
            "clip_none" => {
                window_state::set_clipboard_mode(ClipboardMode::None);
                let _ = clip_none_for_menu.set_checked(true);
                let _ = clip_original_for_menu.set_checked(false);
                let _ = clip_translated_for_menu.set_checked(false);
                let _ = clip_both_for_menu.set_checked(false);
            }
            "clip_original" => {
                window_state::set_clipboard_mode(ClipboardMode::OriginalOnly);
                let _ = clip_none_for_menu.set_checked(false);
                let _ = clip_original_for_menu.set_checked(true);
                let _ = clip_translated_for_menu.set_checked(false);
                let _ = clip_both_for_menu.set_checked(false);
            }
            "clip_translated" => {
                window_state::set_clipboard_mode(ClipboardMode::TranslatedOnly);
                let _ = clip_none_for_menu.set_checked(false);
                let _ = clip_original_for_menu.set_checked(false);
                let _ = clip_translated_for_menu.set_checked(true);
                let _ = clip_both_for_menu.set_checked(false);
            }
            "clip_both" => {
                window_state::set_clipboard_mode(ClipboardMode::Both);
                let _ = clip_none_for_menu.set_checked(false);
                let _ = clip_original_for_menu.set_checked(false);
                let _ = clip_translated_for_menu.set_checked(false);
                let _ = clip_both_for_menu.set_checked(true);
            }
            id if id.starts_with("target_lang_") => {
                if let Some(code) = lang_id_to_code_for_menu.get(id) {
                    let _ = output_lang::set(code);
                    for (lang_code, item) in &lang_items_for_menu {
                        let _ = item.set_checked(lang_code == code);
                    }
                }
            }
            id if id.starts_with("scenario_") => {
                let sid = id.trim_start_matches("scenario_").to_string();
                if crate::scenarios::set_active_scenario_id(sid.clone()).is_ok() {
                    let selected = format!("scenario_{sid}");
                    for item in &scenario_items_for_menu {
                        let _ = item.set_checked(item.id().as_ref() == selected.as_str());
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
    let builder = match icon {
        Some(icon) => builder.icon(icon),
        None => builder,
    };
    let _tray = builder.build(app)?;

    app.listen("window-state-changed", move |event| {
        if let Ok(state) = serde_json::from_str::<WindowState>(event.payload()) {
            let _ = show_popup_item.set_checked(state.popup_show_enabled);
            let _ = clip_none_item.set_checked(state.clipboard_mode == ClipboardMode::None);
            let _ = clip_original_item.set_checked(state.clipboard_mode == ClipboardMode::OriginalOnly);
            let _ = clip_translated_item.set_checked(state.clipboard_mode == ClipboardMode::TranslatedOnly);
            let _ = clip_both_item.set_checked(state.clipboard_mode == ClipboardMode::Both);
        }
    });

    let lang_items_for_listener = lang_items
        .iter()
        .map(|(code, item)| (code.clone(), item.clone()))
        .collect::<Vec<_>>();
    app.listen("output-language-changed", move |event| {
        if let Ok(lang) = serde_json::from_str::<String>(event.payload()) {
            for (code, item) in &lang_items_for_listener {
                let _ = item.set_checked(code == &lang);
            }
        }
    });

    let scenario_items_for_listener = scenario_items.clone();
    app.listen("scenarios-changed", move |_event| {
        let active = crate::scenarios::get_active_scenario_id();
        let selected = format!("scenario_{active}");
        for item in &scenario_items_for_listener {
            let _ = item.set_checked(item.id().as_ref() == selected.as_str());
        }
    });

    Ok(())
}
