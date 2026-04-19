use tauri::{
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, Submenu},
    tray::TrayIconBuilder,
    AppHandle,
};

use crate::output_lang;

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let current_lang = output_lang::current();

    let show_settings = MenuItem::with_id(app, "show_settings", "顯示設定...", true, None::<&str>)?;
    let lang_zh = CheckMenuItem::with_id(
        app,
        "lang_zh",
        "繁體中文",
        true,
        current_lang == "zh",
        None::<&str>,
    )?;
    let lang_en = CheckMenuItem::with_id(
        app,
        "lang_en",
        "English",
        true,
        current_lang == "en",
        None::<&str>,
    )?;
    let lang_submenu = Submenu::with_items(app, "輸出語言", true, &[&lang_zh, &lang_en])?;
    let quit = MenuItem::with_id(app, "quit", "結束", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_settings, &lang_submenu, &quit])?;

    let lang_zh_item = lang_zh.clone();
    let lang_en_item = lang_en.clone();
    let Some(icon) = app.default_window_icon().cloned() else {
        return Ok(());
    };

    let _tray = TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app: &AppHandle, event: MenuEvent| match event.id().as_ref() {
            "show_settings" => {
                let _ = crate::commands::result_window::show_settings_window(app.clone());
            }
            "lang_zh" => {
                let _ = output_lang::set("zh");
                let _ = lang_zh_item.set_checked(true);
                let _ = lang_en_item.set_checked(false);
            }
            "lang_en" => {
                let _ = output_lang::set("en");
                let _ = lang_zh_item.set_checked(false);
                let _ = lang_en_item.set_checked(true);
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}
