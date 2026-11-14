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

pub mod ai;
pub mod appearance;
pub mod backup;
pub mod calendar;
pub mod color_field;
pub mod form;
pub mod format_editor;
pub mod popover;
pub mod projects;
pub mod reports;
pub mod tags;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    Appearance,
    Calendar,
    Reports,
    Projects,
    Tags,
    Ai,
    Backup,
}

impl SettingsSection {
    pub const ALL: [SettingsSection; 7] = [Self::Appearance, Self::Calendar, Self::Reports, Self::Projects, Self::Tags, Self::Ai, Self::Backup];

    pub fn label(self) -> &'static str {
        match self {
            Self::Appearance => "外観",
            Self::Calendar => "カレンダー",
            Self::Reports => "レポート",
            Self::Projects => "プロジェクト",
            Self::Tags => "タグ",
            Self::Ai => "AI",
            Self::Backup => "バックアップと復元",
        }
    }

    /// Group heading in the sidebar (SettingsLayout's categoryGroups, without メモ).
    pub fn group(self) -> &'static str {
        match self {
            Self::Appearance | Self::Calendar | Self::Reports => "ビュー",
            Self::Projects | Self::Tags | Self::Ai => "共通",
            Self::Backup => "データ",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.key() == key)
    }

    fn key(self) -> &'static str {
        match self {
            Self::Appearance => "appearance",
            Self::Calendar => "calendar",
            Self::Reports => "reports",
            Self::Projects => "projects",
            Self::Tags => "tags",
            Self::Ai => "ai",
            Self::Backup => "backup",
        }
    }

    fn icon(self) -> IconName {
        match self {
            Self::Appearance => IconName::Eye,
            Self::Calendar => IconName::Calendar,
            Self::Reports => IconName::ChartPie,
            Self::Projects => IconName::Folder,
            Self::Tags => IconName::Star,
            Self::Ai => IconName::Bot,
            Self::Backup => IconName::HardDrive,
        }
    }
}

pub struct SettingsWindow {
    pub state: Entity<AppState>,
    pub section: SettingsSection,
    /// Section views, created the first time each is shown.
    pub appearance: Option<Entity<appearance::AppearanceSettings>>,
    pub calendar: Option<Entity<calendar::CalendarSettings>>,
    pub reports: Option<Entity<reports::ReportSettings>>,
    pub projects: Option<Entity<projects::ProjectSettings>>,
    pub tags: Option<Entity<tags::TagSettings>>,
    pub ai: Option<Entity<ai::AiSettings>>,
    pub backup: Option<Entity<backup::BackupSettings>>,
}

impl SettingsWindow {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        cx.observe_window_appearance(window, |this, window, cx| this.on_system_appearance(is_dark(window.appearance()), cx)).detach();
        SettingsWindow { state, section: SettingsSection::ALL[0], appearance: None, calendar: None, reports: None, projects: None, tags: None, ai: None, backup: None }
    }

    /// macOS switched light/dark: follow it (when 自動) even if the main window is closed.
    pub fn on_system_appearance(&mut self, system_dark: bool, cx: &mut Context<Self>) {
        let setting = self.state.read(cx).api.full_config().appearance;
        apply_setting(setting, system_dark, cx);
    }

    pub fn show(&mut self, section: SettingsSection, cx: &mut Context<Self>) {
        self.section = section;
        cx.notify();
    }

    fn section_view(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<gpui::AnyView> {
        let state = self.state.clone();
        match self.section {
            SettingsSection::Appearance => Some(self.appearance.get_or_insert_with(|| cx.new(|cx| appearance::AppearanceSettings::new(state, cx))).clone().into()),
            SettingsSection::Calendar => Some(self.calendar.get_or_insert_with(|| cx.new(|cx| calendar::CalendarSettings::new(state, cx))).clone().into()),
            SettingsSection::Reports => {
                if self.reports.is_none() {
                    let r = cx.new(|cx| reports::ReportSettings::new(state, window, cx));
                    // Opening/closing a format editor changes the page (and resets the scroll).
                    cx.observe(&r, |_, _, cx| cx.notify()).detach();
                    self.reports = Some(r);
                }
                self.reports.clone().map(Into::into)
            }
            SettingsSection::Projects => Some(self.projects.get_or_insert_with(|| cx.new(|cx| projects::ProjectSettings::new(state, cx))).clone().into()),
            SettingsSection::Tags => Some(self.tags.get_or_insert_with(|| cx.new(|cx| tags::TagSettings::new(state, cx))).clone().into()),
            SettingsSection::Ai => Some(self.ai.get_or_insert_with(|| cx.new(|cx| ai::AiSettings::new(state, window, cx))).clone().into()),
            SettingsSection::Backup => Some(self.backup.get_or_insert_with(|| cx.new(|cx| backup::BackupSettings::new(state, window, cx))).clone().into()),
        }
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let mut col = div().w(px(200.)).flex_shrink_0().h_full().flex().flex_col().gap(px(1.)).px_2().bg(p.surface).border_r_1().border_color(p.border);
        // The row with the traffic lights drags the window (the page beside it starts at the top).
        col = col.child(
            div()
                .id("settings-drag")
                .debug_selector(|| "settings-drag".into())
                .flex_shrink_0()
                .h(px(40.))
                .on_mouse_down(gpui::MouseButton::Left, |ev, window, _| {
                    if ev.click_count == 2 {
                        window.titlebar_double_click();
                    } else {
                        window.start_window_move();
                    }
                }),
        );
        let mut last_group = "";
        for s in SettingsSection::ALL {
            // Groups are set apart by a little space, without headings.
            if s.group() != last_group {
                if !last_group.is_empty() {
                    col = col.child(div().h(px(6.)));
                }
                last_group = s.group();
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
                    .py(px(4.))
                    .rounded_md()
                    .text_sm()
                    .cursor_pointer()
                    .when(active, |d| d.bg(p.selected).font_weight(FontWeight::MEDIUM).text_color(p.text))
                    .when(!active, |d| d.text_color(p.text).hover(|h| h.bg(p.hover)))
                    .child(Icon::new(s.icon()).size_4())
                    .child(s.label())
                    .on_click(cx.listener(move |this, _, _, cx| this.show(s, cx))),
            );
        }
        col
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let content = self.section_view(window, cx);
        let editing_format = self.reports.as_ref().is_some_and(|r| r.read(cx).editing.is_some());
        div()
            .id("settings-window")
            .size_full()
            .flex()
            .bg(p.bg)
            .text_color(p.text)
            .font_family(".SystemUIFont")
            .child(self.render_sidebar(cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(if self.section == SettingsSection::Projects {
                        // The projects page fills the height and scrolls its two panes itself.
                        div().flex_1().min_h_0().children(content).into_any_element()
                    } else {
                        div()
                            // A new id per page starts it scrolled to the top (Tauri navigates between pages).
                            .id(gpui::SharedString::from(format!("settings-content-{}-{}", self.section.key(), editing_format)))
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .px_8()
                            .pt_6()
                            .pb_8()
                            .children(content)
                            .into_any_element()
                    }),
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
            titlebar: Some(TitlebarOptions { title: Some("設定".into()), appears_transparent: true, traffic_light_position: Some(crate::ui::theme::traffic_light_position()) }),
            window_min_size: Some(size(px(640.), px(420.))),
            ..Default::default()
        },
        |window, cx| {
            let view = cx.new(|cx| SettingsWindow::new(state, window, cx));
            cx.set_global(OpenSettings { window: window.window_handle(), view: view.downgrade() });
            cx.new(|cx| Root::new(view, window, cx))
        },
    );
    if let Ok(handle) = opened {
        let _ = handle.update(cx, |_, window, _| window.activate_window());
    }
}
