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

use gpui::{App, AppContext, Bounds, Point, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use gpui_component::Root;

fn main() {
    gpui_platform::application().with_assets(gpui_kit_assets::Assets::new("")).run(|cx: &mut App| {
        gpui_component::init(cx);
        crate::ui::theme::install_light_theme(cx);
        actions::bind_keys(cx);
        actions::set_menus(cx);
        cx.on_action(|_: &actions::Quit, cx| cx.quit());
        // Dev/measurement: TRACK_GPUI_WINDOW=1440x900 sets the initial window size.
        let (w, h) = std::env::var("TRACK_GPUI_WINDOW")
            .ok()
            .and_then(|v| v.split_once('x').and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?))))
            .unwrap_or((1200., 800.));
        let bounds = Bounds::centered(None, size(px(w), px(h)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Track (GPUI)".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(Point::new(px(14.), px(14.))),
                }),
                window_min_size: Some(size(px(360.), px(480.))),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| app::TrackApp::new(window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
