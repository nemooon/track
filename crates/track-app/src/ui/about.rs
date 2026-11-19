//! 「Track について」: a small window of its own, like a Mac app's About panel — the name, the
//! version and a link to the repository. Opened from the header's menu and the Track menu.
use crate::ui::theme::*;
use gpui::{
    AnyWindowHandle, App, AppContext, Context, Global, InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled, TitlebarOptions,
    WeakEntity, Window, WindowBounds, WindowOptions, div, px, size,
};
use gpui_component::Root;

pub const GITHUB_URL: &str = "https://github.com/nemooon/track";

/// The app icon (rendered by scripts/build-icon.sh).
const ICON_PNG: &[u8] = include_bytes!("../../../../packaging/icon-256.png");

pub struct AboutWindow {
    icon: std::sync::Arc<gpui::Image>,
}

struct OpenAbout {
    window: AnyWindowHandle,
    view: WeakEntity<AboutWindow>,
}

impl Global for OpenAbout {}

/// The open About window, if any.
pub fn about_window(cx: &App) -> Option<(AnyWindowHandle, gpui::Entity<AboutWindow>)> {
    let open = cx.try_global::<OpenAbout>()?;
    let view = open.view.upgrade()?;
    cx.windows().contains(&open.window).then_some((open.window, view))
}

/// Opens the About window, or brings the open one forward.
pub fn open_about(cx: &mut App) {
    if let Some((window, _)) = about_window(cx) {
        let _ = window.update(cx, |_, window, _| window.activate_window());
        return;
    }
    let bounds = gpui::Bounds::centered(None, size(px(320.), px(300.)), cx);
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions { title: Some("Track について".into()), appears_transparent: true, traffic_light_position: Some(traffic_light_position()) }),
            is_resizable: false,
            is_minimizable: false,
            ..Default::default()
        },
        |window, cx| {
            let view = cx.new(|_| AboutWindow { icon: std::sync::Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Png, ICON_PNG.to_vec())) });
            cx.set_global(OpenAbout { window: window.window_handle(), view: view.downgrade() });
            cx.new(|cx| Root::new(view, window, cx))
        },
    );
    if let Ok(handle) = opened {
        let _ = handle.update(cx, |_, window, _| window.activate_window());
    }
}

impl Render for AboutWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_1()
            .bg(p.bg)
            .text_color(p.text)
            .child(gpui::img(self.icon.clone()).debug_selector(|| "about-icon".into()).size(px(96.)).mb_2())
            .child(div().text_size(px(20.)).font_weight(gpui::FontWeight::BOLD).child("Track"))
            .child(div().debug_selector(|| "about-version".into()).text_size(px(12.)).text_color(p.text_muted).child(format!("バージョン {}", env!("CARGO_PKG_VERSION"))))
            .child(div().mt_2().text_size(px(12.)).text_color(p.text_muted).child("ローカルで動く工数記録アプリ"))
            .child(
                div()
                    .id("about-github")
                    .debug_selector(|| "about-github".into())
                    .mt_3()
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .text_size(px(12.))
                    .text_color(p.text_muted)
                    .cursor_pointer()
                    .hover(move |s| s.text_color(p.text))
                    .child(crate::ui::icons::github().size(px(14.)))
                    .child("GitHub")
                    .on_click(|_, _, cx| cx.open_url(GITHUB_URL)),
            )
    }
}
