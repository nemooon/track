#![allow(dead_code)] // prototype: modules land before their callers
mod actions;
mod api;
mod app;
mod calendar;
mod core;
mod persist;
mod reports;
mod ui;
#[cfg(test)]
mod testing;
#[cfg(test)]
mod app_tests;
#[cfg(test)]
mod calendar_tests;
#[cfg(test)]
mod reports_tests;

use gpui::{App, AppContext, Bounds, Point, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use gpui_component::Root;

fn main() {
    gpui_platform::application().with_assets(gpui_kit_assets::Assets::new("")).run(|cx: &mut App| {
        gpui_component::init(cx);
        actions::bind_keys(cx);
        actions::set_menus(cx);
        cx.on_action(|_: &actions::Quit, cx| cx.quit());
        let bounds = Bounds::centered(None, size(px(1200.), px(800.)), cx);
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
