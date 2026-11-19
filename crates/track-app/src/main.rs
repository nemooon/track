mod actions;
mod app;
mod calendar;
mod core;
mod data;
mod persist;
mod reports;
mod settings;
mod ui;
#[cfg(test)]
mod testing;
#[cfg(test)]
mod app_tests;
#[cfg(test)]
mod calendar_tests;
#[cfg(test)]
mod reports_tests;
#[cfg(test)]
mod settings_tests;

use gpui::{App, AppContext};
use gpui_component::Root;

fn main() {
    let application = gpui_platform::application().with_assets(crate::ui::icons::AppAssets(gpui_kit_assets::Assets::new("")));
    // Clicking the Dock icon with the main window closed opens it again.
    application.on_reopen(|cx| app::show_main_window(None, cx));
    application.run(|cx: &mut App| {
        gpui_component::init(cx);
        let setting = track_core::config::load_config(&track_core::guard::data_dir_from_env()).appearance;
        crate::ui::theme::apply_setting(setting, crate::ui::theme::is_dark(cx.window_appearance()), cx);
        actions::bind_keys(cx);
        actions::set_menus(cx);
        #[cfg(target_os = "macos")]
        actions::register_system_menus();
        cx.on_action(|_: &actions::Quit, cx| cx.quit());
        // From the Track menu even while another window (settings, about) is in front.
        cx.on_action(|_: &actions::ShowAbout, cx| crate::ui::about::open_about(cx));
        cx.on_action(|_: &actions::CheckForUpdates, cx| crate::ui::update::open_update(cx));
        let options = app::main_window_options(cx);
        cx.open_window(options, |window, cx| {
            let view = cx.new(|cx| app::TrackApp::new(window, cx));
            cx.new(|cx| Root::new(view, window, cx))
        })
        .unwrap();
        cx.activate(true);
    });
}
