use gpui::{App, KeyBinding, Menu, MenuItem, OsAction, SystemMenuType, actions};

actions!(track, [ShowCalendar, ShowReports, PrevPeriod, NextPeriod, Today, ZoomIn, ZoomOut, InstallIntegrations, OpenSettings, ShowAbout, CheckForUpdates, CloseWindow, Quit]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-1", ShowCalendar, None),
        KeyBinding::new("cmd-2", ShowReports, None),
        KeyBinding::new("cmd-[", PrevPeriod, None),
        KeyBinding::new("cmd-]", NextPeriod, None),
        KeyBinding::new("cmd-t", Today, None),
        KeyBinding::new("cmd-=", ZoomIn, None),
        KeyBinding::new("cmd-+", ZoomIn, None),
        KeyBinding::new("cmd--", ZoomOut, None),
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-,", OpenSettings, None),
        // Closes the window in front when it handles it (the skills window); the main window doesn't.
        KeyBinding::new("cmd-w", CloseWindow, None),
    ]);
    // ⌘W closes the window in front, the main window included.
    cx.on_action(|_: &CloseWindow, cx| {
        if let Some(window) = cx.active_window() {
            // The key went through that window, which is still busy: close it right after.
            cx.defer(move |cx| {
                let _ = window.update(cx, |_, window, _| window.remove_window());
            });
        }
    });
    // ⌘1・⌘2 from another window, or with the main window closed: bring it (back) on that page.
    cx.on_action(|_: &ShowCalendar, cx| crate::app::show_main_window(Some(crate::app::Page::Calendar), cx));
    cx.on_action(|_: &ShowReports, cx| crate::app::show_main_window(Some(crate::app::Page::Reports), cx));
}

/// Japanese native menus (the standard ones plus 「表示」 for pages, periods and zoom).
pub fn set_menus(cx: &mut App) {
    cx.set_menus([
        Menu::new("Track").items([
            MenuItem::action("Track について", ShowAbout),
            MenuItem::action("アップデートを確認…", CheckForUpdates),
            MenuItem::separator(),
            MenuItem::action("設定…", OpenSettings),
            MenuItem::separator(),
            MenuItem::action(if track_core::guard::GUARDED { "スキルのインストール…（開発用）" } else { "スキルのインストール…" }, InstallIntegrations),
            MenuItem::separator(),
            MenuItem::os_submenu("サービス", SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action("Track を終了", Quit),
        ]),
        Menu::new("編集").items([
            MenuItem::os_action("取り消す", gpui_component::input::Undo, OsAction::Undo),
            MenuItem::os_action("やり直す", gpui_component::input::Redo, OsAction::Redo),
            MenuItem::separator(),
            MenuItem::os_action("カット", gpui_component::input::Cut, OsAction::Cut),
            MenuItem::os_action("コピー", gpui_component::input::Copy, OsAction::Copy),
            MenuItem::os_action("ペースト", gpui_component::input::Paste, OsAction::Paste),
            MenuItem::os_action("すべてを選択", gpui_component::input::SelectAll, OsAction::SelectAll),
        ]),
        Menu::new("表示").items([
            MenuItem::action("カレンダー", ShowCalendar),
            MenuItem::action("レポート", ShowReports),
            MenuItem::separator(),
            MenuItem::action("前の期間", PrevPeriod),
            MenuItem::action("次の期間", NextPeriod),
            MenuItem::action("今日", Today),
            MenuItem::separator(),
            MenuItem::action("カレンダーを拡大", ZoomIn),
            MenuItem::action("カレンダーを縮小", ZoomOut),
        ]),
    ]);
}
