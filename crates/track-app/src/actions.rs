use gpui::{App, KeyBinding, Menu, MenuItem, OsAction, SystemMenuType, actions};

actions!(track, [ShowCalendar, ShowReports, PrevPeriod, NextPeriod, Today, ZoomIn, ZoomOut, InstallIntegrations, OpenSettings, ShowAbout, CheckForUpdates, CloseWindow, ShowMainWindow, Minimize, Zoom, ToggleFullScreen, BringAllToFront, ReportIssue, Quit]);

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
        KeyBinding::new("cmd-0", ShowMainWindow, None),
        KeyBinding::new("cmd-m", Minimize, None),
        KeyBinding::new("ctrl-cmd-f", ToggleFullScreen, None),
        // In the calendar: ⌘A selects the shown entries, ⌘Z brings back the last deletion, ⌘⇧Z
        // deletes it again.
        KeyBinding::new("cmd-a", gpui_component::input::SelectAll, Some("Calendar")),
        KeyBinding::new("cmd-z", gpui_component::input::Undo, Some("Calendar")),
        KeyBinding::new("cmd-shift-z", gpui_component::input::Redo, Some("Calendar")),
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
    // The Window menu acts on the window in front (after the key or menu is done with it).
    fn on_front(cx: &mut App, f: impl FnOnce(&mut gpui::Window) + 'static) {
        if let Some(window) = cx.active_window() {
            cx.defer(move |cx| {
                let _ = window.update(cx, |_, window, _| f(window));
            });
        }
    }
    cx.on_action(|_: &Minimize, cx| on_front(cx, |w| w.minimize_window()));
    cx.on_action(|_: &Zoom, cx| on_front(cx, |w| w.zoom_window()));
    cx.on_action(|_: &ToggleFullScreen, cx| on_front(cx, |w| w.toggle_fullscreen()));
    cx.on_action(|_: &BringAllToFront, cx| cx.activate(true));
    cx.on_action(|_: &ReportIssue, cx| cx.open_url(&issue_url(env!("CARGO_PKG_VERSION"), &macos_version())));
    // ⌘0: the main window as it was (the Window menu's 「Track」).
    cx.on_action(|_: &ShowMainWindow, cx| crate::app::show_main_window(None, cx));
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
        Menu::new("ウインドウ").items([
            MenuItem::action("しまう", Minimize),
            MenuItem::action("拡大／縮小", Zoom),
            // macOS adds 「フルスクリーンにする」, tiling and the window list to this menu itself
            // (register_system_menus); ⌃⌘F still works.
            MenuItem::separator(),
            MenuItem::action("メインウインドウ", ShowMainWindow),
            MenuItem::separator(),
            MenuItem::action("すべてを手前に移動", BringAllToFront),
        ]),
        Menu::new("ヘルプ").items([MenuItem::action("問題を報告…", ReportIssue)]),
    ]);
}

/// A new GitHub issue for Track, with the app and macOS versions filled in (nothing else).
pub fn issue_url(version: &str, os: &str) -> String {
    let body = format!("## 起きたこと\n\n\n## 手順\n\n1. \n\n## 環境\n\n- Track {version}\n- {os}\n");
    format!("https://github.com/nemooon/track/issues/new?body={}", encode(&body))
}

/// Percent-encodes everything but unreserved characters.
fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// 「macOS 15.1」 from sw_vers, or just 「macOS」.
fn macos_version() -> String {
    std::process::Command::new("/usr/bin/sw_vers")
        .arg("-productVersion")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|v| format!("macOS {}", v.trim()))
        .unwrap_or_else(|| "macOS".into())
}

/// Tells macOS which of our (Japanese) menus are the Window and Help menus, so it adds the window
/// list and tiling to 「ウインドウ」 and the search field to 「ヘルプ」. GPUI does this only for a
/// menu named "Window". Call right after `set_menus`.
#[cfg(target_os = "macos")]
pub fn register_system_menus() {
    use objc2_app_kit::NSApplication;
    use objc2_foundation::{MainThreadMarker, NSString};
    let Some(mtm) = MainThreadMarker::new() else { return };
    let app = NSApplication::sharedApplication(mtm);
    let Some(main) = app.mainMenu() else { return };
    let submenu = |title: &str| main.itemWithTitle(&NSString::from_str(title)).and_then(|item| item.submenu());
    if let Some(menu) = submenu("ウインドウ") {
        // Point our items at AppKit's own actions, so macOS sees the standard 「しまう」
        // 「拡大／縮小」「すべてを手前に移動」 and puts its items (fill, tiling, the window
        // list) around them in the usual order.
        for (title, action) in [("しまう", objc2::sel!(performMiniaturize:)), ("拡大／縮小", objc2::sel!(performZoom:)), ("すべてを手前に移動", objc2::sel!(arrangeInFront:))] {
            if let Some(item) = menu.itemWithTitle(&NSString::from_str(title)) {
                unsafe {
                    item.setTarget(None);
                    item.setAction(Some(action));
                }
            }
        }
        app.setWindowsMenu(Some(&menu));
    }
    if let Some(menu) = submenu("ヘルプ") {
        app.setHelpMenu(Some(&menu));
    }
}
