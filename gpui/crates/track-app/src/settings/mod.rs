//! The settings window (Tauri: SettingsLayout + SettingsPage + ProjectsPage + ReportCopyFormatPage).
//! A second window with the section list on the left; opened by ⌘, , the Track menu or the
//! header button, one at a time.
use crate::app::AppState;
use crate::ui::theme::*;
use gpui::{
    AnyWindowHandle, App, AppContext, Bounds, Context, Entity, FontWeight, Global, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, TitlebarOptions, WeakEntity, Window, WindowBounds, WindowOptions, div, prelude::FluentBuilder, px, size,
};
use gpui_component::{Icon, IconName, Root};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    Calendar,
    Reports,
    Projects,
    Ai,
    Backup,
}

impl SettingsSection {
    pub const ALL: [SettingsSection; 5] = [Self::Calendar, Self::Reports, Self::Projects, Self::Ai, Self::Backup];

    pub fn label(self) -> &'static str {
        match self {
            Self::Calendar => "カレンダー",
            Self::Reports => "レポート",
            Self::Projects => "プロジェクト",
            Self::Ai => "AI",
            Self::Backup => "バックアップと復元",
        }
    }

    /// Group heading in the sidebar (SettingsLayout's categoryGroups, without メモ).
    pub fn group(self) -> &'static str {
        match self {
            Self::Calendar | Self::Reports => "ビュー",
            Self::Projects | Self::Ai => "共通",
            Self::Backup => "データ",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Calendar => "calendar",
            Self::Reports => "reports",
            Self::Projects => "projects",
            Self::Ai => "ai",
            Self::Backup => "backup",
        }
    }

    fn icon(self) -> IconName {
        match self {
            Self::Calendar => IconName::Calendar,
            Self::Reports => IconName::ChartPie,
            Self::Projects => IconName::Folder,
            Self::Ai => IconName::Bot,
            Self::Backup => IconName::HardDrive,
        }
    }
}

pub struct SettingsWindow {
    pub state: Entity<AppState>,
    pub section: SettingsSection,
}

impl SettingsWindow {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        SettingsWindow { state, section: SettingsSection::Calendar }
    }

    pub fn show(&mut self, section: SettingsSection, cx: &mut Context<Self>) {
        self.section = section;
        cx.notify();
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut col = div().w(px(200.)).flex_shrink_0().h_full().flex().flex_col().gap_1().pt(px(44.)).px_2().bg(n50()).border_r_1().border_color(n200());
        let mut last_group = "";
        for s in SettingsSection::ALL {
            if s.group() != last_group {
                last_group = s.group();
                col = col.child(div().px_2().pt_3().pb_1().text_xs().font_weight(FontWeight::MEDIUM).text_color(n500()).child(last_group));
            }
            let active = self.section == s;
            let id = format!("settings-nav-{}", s.key());
            col = col.child(
                div()
                    .id(gpui::SharedString::from(id.clone()))
                    .debug_selector(move || id.clone())
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py(px(6.))
                    .rounded_md()
                    .text_sm()
                    .cursor_pointer()
                    .when(active, |d| d.bg(n200()).font_weight(FontWeight::MEDIUM).text_color(n900()))
                    .when(!active, |d| d.text_color(n700()).hover(|h| h.bg(n100())))
                    .child(Icon::new(s.icon()).size_4())
                    .child(s.label())
                    .on_click(cx.listener(move |this, _, _, cx| this.show(s, cx))),
            );
        }
        col
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("settings-window")
            .size_full()
            .flex()
            .bg(white())
            .text_color(n900())
            .font_family(".SystemUIFont")
            .child(self.render_sidebar(cx))
            .child(
                div()
                    .id("settings-content")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_y_scroll()
                    .px_8()
                    .pt(px(44.))
                    .pb_8()
                    .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).mb_6().child(self.section.label())),
            )
    }
}

struct OpenSettings {
    window: AnyWindowHandle,
    view: WeakEntity<SettingsWindow>,
}

impl Global for OpenSettings {}

/// The open settings window and its view, if any.
pub fn settings_window(cx: &App) -> Option<(AnyWindowHandle, Entity<SettingsWindow>)> {
    let open = cx.try_global::<OpenSettings>()?;
    let view = open.view.upgrade()?;
    cx.windows().contains(&open.window).then_some((open.window, view))
}

/// Opens the settings window, or brings the open one forward.
pub fn open_settings(state: Entity<AppState>, cx: &mut App) {
    if let Some((window, _)) = settings_window(cx) {
        let _ = window.update(cx, |_, window, _| window.activate_window());
        return;
    }
    let bounds = Bounds::centered(None, size(px(880.), px(640.)), cx);
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions { title: Some("設定".into()), appears_transparent: true, traffic_light_position: Some(gpui::Point::new(px(14.), px(14.))) }),
            window_min_size: Some(size(px(640.), px(420.))),
            ..Default::default()
        },
        |window, cx| {
            let view = cx.new(|cx| SettingsWindow::new(state, cx));
            cx.set_global(OpenSettings { window: window.window_handle(), view: view.downgrade() });
            cx.new(|cx| Root::new(view, window, cx))
        },
    );
    if let Ok(handle) = opened {
        let _ = handle.update(cx, |_, window, _| window.activate_window());
    }
}
