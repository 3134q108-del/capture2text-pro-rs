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
    let available_langs = output_lang::available_langs();
    let has_de = available_langs.iter().any(|lang| lang == "de-DE");
    let has_fr = available_langs.iter().any(|lang| lang == "fr-FR");
    let state = window_state::get();
    let normalized_lang = match current_lang.as_str() {
        "zh-TW" | "zh-CN" | "en-US" | "ja-JP" | "ko-KR" | "de-DE" | "fr-FR" => {
            current_lang.as_str()
        }
        _ => "zh-TW",
    };

    let show_settings = MenuItem::with_id(app, "show_settings", "設定...", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let toggle_show_popup = CheckMenuItem::with_id(
        app,
        "toggle_show_popup",
        "顯示彈窗",
        true,
        state.popup_show_enabled,
        None::<&str>,
    )?;
    let clip_none = CheckMenuItem::with_id(
        app,
        "clip_none",
        "不複製",
        true,
        state.clipboard_mode == ClipboardMode::None,
        None::<&str>,
    )?;
    let clip_original = CheckMenuItem::with_id(
        app,
        "clip_original",
        "只複製原文",
        true,
        state.clipboard_mode == ClipboardMode::OriginalOnly,
        None::<&str>,
    )?;
    let clip_translated = CheckMenuItem::with_id(
        app,
        "clip_translated",
        "只複製譯文",
        true,
        state.clipboard_mode == ClipboardMode::TranslatedOnly,
        None::<&str>,
    )?;
    let clip_both = CheckMenuItem::with_id(
        app,
        "clip_both",
        "複製原文+譯文",
        true,
        state.clipboard_mode == ClipboardMode::Both,
        None::<&str>,
    )?;
    let clip_submenu = Submenu::with_items(
        app,
        "儲存到剪貼簿",
        true,
        &[&clip_none, &clip_original, &clip_translated, &clip_both],
    )?;
    let sep2 = PredefinedMenuItem::separator(app)?;

    let lang_zh_tw = CheckMenuItem::with_id(
        app,
        "lang_zh_tw",
        "繁體中文",
        true,
        normalized_lang == "zh-TW",
        None::<&str>,
    )?;
    let lang_zh_cn = CheckMenuItem::with_id(
        app,
        "lang_zh_cn",
        "簡體中文",
        true,
        normalized_lang == "zh-CN",
        None::<&str>,
    )?;
    let lang_en_us = CheckMenuItem::with_id(
        app,
        "lang_en_us",
        "English",
        true,
        normalized_lang == "en-US",
        None::<&str>,
    )?;
    let lang_ja_jp = CheckMenuItem::with_id(
        app,
        "lang_ja_jp",
        "日本語",
        true,
        normalized_lang == "ja-JP",
        None::<&str>,
    )?;
    let lang_ko_kr = CheckMenuItem::with_id(
        app,
        "lang_ko_kr",
        "한국어",
        true,
        normalized_lang == "ko-KR",
        None::<&str>,
    )?;
    let lang_de_de = CheckMenuItem::with_id(
        app,
        "lang_de_de",
        "Deutsch",
        has_de,
        normalized_lang == "de-DE",
        None::<&str>,
    )?;
    let lang_fr_fr = CheckMenuItem::with_id(
        app,
        "lang_fr_fr",
        "Français",
        has_fr,
        normalized_lang == "fr-FR",
        None::<&str>,
    )?;
    let lang_submenu = Submenu::with_items(
        app,
        "輸出語言",
        true,
        &[
            &lang_zh_tw,
            &lang_zh_cn,
            &lang_en_us,
            &lang_ja_jp,
            &lang_ko_kr,
            &lang_de_de,
            &lang_fr_fr,
        ],
    )?;

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
    let quit = MenuItem::with_id(app, "quit", "結束", true, None::<&str>)?;
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
    let lang_zh_tw_item = lang_zh_tw.clone();
    let lang_zh_cn_item = lang_zh_cn.clone();
    let lang_en_us_item = lang_en_us.clone();
    let lang_ja_jp_item = lang_ja_jp.clone();
    let lang_ko_kr_item = lang_ko_kr.clone();
    let lang_de_de_item = lang_de_de.clone();
    let lang_fr_fr_item = lang_fr_fr.clone();

    let show_popup_for_menu = show_popup_item.clone();
    let clip_none_for_menu = clip_none_item.clone();
    let clip_original_for_menu = clip_original_item.clone();
    let clip_translated_for_menu = clip_translated_item.clone();
    let clip_both_for_menu = clip_both_item.clone();
    let lang_zh_tw_for_menu = lang_zh_tw_item.clone();
    let lang_zh_cn_for_menu = lang_zh_cn_item.clone();
    let lang_en_us_for_menu = lang_en_us_item.clone();
    let lang_ja_jp_for_menu = lang_ja_jp_item.clone();
    let lang_ko_kr_for_menu = lang_ko_kr_item.clone();
    let lang_de_de_for_menu = lang_de_de_item.clone();
    let lang_fr_fr_for_menu = lang_fr_fr_item.clone();
    let scenario_items_for_menu = scenario_items.clone();

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
            "lang_zh_tw" => {
                let _ = output_lang::set("zh-TW");
                let _ = lang_zh_tw_for_menu.set_checked(true);
                let _ = lang_zh_cn_for_menu.set_checked(false);
                let _ = lang_en_us_for_menu.set_checked(false);
                let _ = lang_ja_jp_for_menu.set_checked(false);
                let _ = lang_ko_kr_for_menu.set_checked(false);
                let _ = lang_de_de_for_menu.set_checked(false);
                let _ = lang_fr_fr_for_menu.set_checked(false);
            }
            "lang_zh_cn" => {
                let _ = output_lang::set("zh-CN");
                let _ = lang_zh_tw_for_menu.set_checked(false);
                let _ = lang_zh_cn_for_menu.set_checked(true);
                let _ = lang_en_us_for_menu.set_checked(false);
                let _ = lang_ja_jp_for_menu.set_checked(false);
                let _ = lang_ko_kr_for_menu.set_checked(false);
                let _ = lang_de_de_for_menu.set_checked(false);
                let _ = lang_fr_fr_for_menu.set_checked(false);
            }
            "lang_en_us" => {
                let _ = output_lang::set("en-US");
                let _ = lang_zh_tw_for_menu.set_checked(false);
                let _ = lang_zh_cn_for_menu.set_checked(false);
                let _ = lang_en_us_for_menu.set_checked(true);
                let _ = lang_ja_jp_for_menu.set_checked(false);
                let _ = lang_ko_kr_for_menu.set_checked(false);
                let _ = lang_de_de_for_menu.set_checked(false);
                let _ = lang_fr_fr_for_menu.set_checked(false);
            }
            "lang_ja_jp" => {
                let _ = output_lang::set("ja-JP");
                let _ = lang_zh_tw_for_menu.set_checked(false);
                let _ = lang_zh_cn_for_menu.set_checked(false);
                let _ = lang_en_us_for_menu.set_checked(false);
                let _ = lang_ja_jp_for_menu.set_checked(true);
                let _ = lang_ko_kr_for_menu.set_checked(false);
                let _ = lang_de_de_for_menu.set_checked(false);
                let _ = lang_fr_fr_for_menu.set_checked(false);
            }
            "lang_ko_kr" => {
                let _ = output_lang::set("ko-KR");
                let _ = lang_zh_tw_for_menu.set_checked(false);
                let _ = lang_zh_cn_for_menu.set_checked(false);
                let _ = lang_en_us_for_menu.set_checked(false);
                let _ = lang_ja_jp_for_menu.set_checked(false);
                let _ = lang_ko_kr_for_menu.set_checked(true);
                let _ = lang_de_de_for_menu.set_checked(false);
                let _ = lang_fr_fr_for_menu.set_checked(false);
            }
            "lang_de_de" => {
                let _ = output_lang::set("de-DE");
                let _ = lang_zh_tw_for_menu.set_checked(false);
                let _ = lang_zh_cn_for_menu.set_checked(false);
                let _ = lang_en_us_for_menu.set_checked(false);
                let _ = lang_ja_jp_for_menu.set_checked(false);
                let _ = lang_ko_kr_for_menu.set_checked(false);
                let _ = lang_de_de_for_menu.set_checked(true);
                let _ = lang_fr_fr_for_menu.set_checked(false);
            }
            "lang_fr_fr" => {
                let _ = output_lang::set("fr-FR");
                let _ = lang_zh_tw_for_menu.set_checked(false);
                let _ = lang_zh_cn_for_menu.set_checked(false);
                let _ = lang_en_us_for_menu.set_checked(false);
                let _ = lang_ja_jp_for_menu.set_checked(false);
                let _ = lang_ko_kr_for_menu.set_checked(false);
                let _ = lang_de_de_for_menu.set_checked(false);
                let _ = lang_fr_fr_for_menu.set_checked(true);
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

    let lang_items = vec![
        ("zh-TW", lang_zh_tw_item),
        ("zh-CN", lang_zh_cn_item),
        ("en-US", lang_en_us_item),
        ("ja-JP", lang_ja_jp_item),
        ("ko-KR", lang_ko_kr_item),
        ("de-DE", lang_de_de_item),
        ("fr-FR", lang_fr_fr_item),
    ];
    app.listen("output-language-changed", move |event| {
        if let Ok(lang) = serde_json::from_str::<String>(event.payload()) {
            for (code, item) in &lang_items {
                let _ = item.set_checked(*code == lang);
            }
        }
    });

    let lang_de_de_enable = lang_de_de.clone();
    let lang_fr_fr_enable = lang_fr_fr.clone();
    app.listen("pixtral-install-done", move |_event| {
        let _ = lang_de_de_enable.set_enabled(true);
        let _ = lang_fr_fr_enable.set_enabled(true);
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
