use crate::actions::*;
use crate::api::{ApiClient, AppConfig, Client, Project, Settings, Tag};
use crate::calendar::CalendarView;
use crate::persist::PersistState;
use crate::reports::ReportsView;
use crate::ui::theme::*;
use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Render, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{Icon, IconName};
use std::path::PathBuf;

/// Reference data both pages need (`/projects`, `/clients`, `/tags`, `/settings`, `/config`).
#[derive(Clone)]
pub struct AppData {
    pub projects: Vec<Project>,
    pub clients: Vec<Client>,
    pub tags: Vec<Tag>,
    pub settings: Settings,
    pub config: AppConfig,
}

pub struct AppState {
    pub api: ApiClient,
    pub data: Option<AppData>,
    pub error: Option<String>,
    pub loading: bool,
    pub persist: PersistState,
    persist_path: PathBuf,
}

impl AppState {
    pub fn new(api: ApiClient) -> Self {
        Self::with_path(api, PersistState::default_path())
    }

    pub fn with_path(api: ApiClient, persist_path: PathBuf) -> Self {
        Self { api, data: None, error: None, loading: false, persist: PersistState::load(&persist_path), persist_path }
    }

    pub fn update_persist(&mut self, f: impl FnOnce(&mut PersistState)) {
        f(&mut self.persist);
        let _ = self.persist.save(&self.persist_path);
    }

    pub fn load(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        self.error = None;
        cx.notify();
        let api = self.api.clone();
        cx.spawn(async move |this, cx| {
            let result = async {
                Ok::<_, crate::api::ApiError>(AppData {
                    projects: api.projects().await?,
                    clients: api.clients().await?,
                    tags: api.tags().await?,
                    settings: api.settings().await?,
                    config: api.config().await?,
                })
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                match result {
                    Ok(data) => this.data = Some(data),
                    Err(e) => this.error = Some(e.message),
                }
                cx.notify();
            });
        })
        .detach();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Calendar,
    Reports,
}

pub struct TrackApp {
    pub state: Entity<AppState>,
    pub page: Page,
    pub calendar: Entity<CalendarView>,
    pub reports: Entity<ReportsView>,
    focus: FocusHandle,
}

impl TrackApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|_| AppState::new(ApiClient::from_env()));
        state.update(cx, |s, cx| s.load(cx));
        Self::with_state(state, window, cx)
    }

    pub fn with_state(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let calendar = cx.new(|cx| CalendarView::new(state.clone(), window, cx));
        let reports = cx.new(|cx| ReportsView::new(state.clone(), window, cx));
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        Self { state, page: Page::Calendar, calendar, reports, focus }
    }

    fn show(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        cx.notify();
    }

    fn render_header(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = |id: &'static str, label: &'static str, icon: IconName, page: Page, active: bool| {
            div()
                .id(id)
                .relative()
                .h_full()
                .flex()
                .items_center()
                .gap(px(6.))
                .px_3()
                .text_xs()
                .font_weight(gpui::FontWeight::MEDIUM)
                .cursor_pointer()
                .text_color(if active { white().into() } else { with_alpha(white(), 0.55) })
                .when(!active, |d| d.hover(|s| s.text_color(with_alpha(white(), 0.85))))
                .child(Icon::new(icon).size_4())
                .child(label)
                .when(active, |d| d.child(div().absolute().left_3().right_3().bottom_0().h(px(2.)).rounded_full().bg(white())))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _, _, cx| this.show(page, cx)))
        };
        let fullscreen = window.is_fullscreen();
        div()
            .id("header")
            .flex()
            .flex_shrink_0()
            .items_center()
            .h(px(44.))
            .pr_3()
            .pl(px(if fullscreen { 12. } else { 92. }))
            .bg(header_bg())
            .border_b_1()
            .border_color(header_border())
            .on_mouse_down(MouseButton::Left, |ev, window, _| {
                if ev.click_count == 2 {
                    window.titlebar_double_click();
                } else {
                    window.start_window_move();
                }
            })
            .child(
                div()
                    .h_full()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(tab("tab-calendar", "カレンダー", IconName::Calendar, Page::Calendar, self.page == Page::Calendar))
                    .child(tab("tab-reports", "レポート", IconName::ChartPie, Page::Reports, self.page == Page::Reports)),
            )
    }

    fn render_body(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let state = self.state.read(cx);
        if state.data.is_none() {
            let error = state.error.clone();
            let loading = state.loading;
            return div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_3()
                .text_sm()
                .text_color(n600())
                .map(|d| match error {
                    Some(e) if !loading => d
                        .child(div().font_weight(gpui::FontWeight::SEMIBOLD).text_color(n800()).child("サーバーに接続できません"))
                        .child(div().text_xs().text_color(n500()).child(e))
                        .child(
                            div()
                                .id("retry")
                                .px_3()
                                .py_1()
                                .rounded_md()
                                .bg(n900())
                                .text_color(white())
                                .text_xs()
                                .cursor_pointer()
                                .child("再試行")
                                .on_click(cx.listener(|this, _, _, cx| this.state.update(cx, |s, cx| s.load(cx)))),
                        ),
                    _ => d.child("読み込み中…"),
                })
                .into_any_element();
        }
        match self.page {
            Page::Calendar => self.calendar.clone().into_any_element(),
            Page::Reports => self.reports.clone().into_any_element(),
        }
    }
}

impl Focusable for TrackApp {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for TrackApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("track-app")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(white())
            .text_color(n900())
            .font_family(".SystemUIFont")
            .on_action(cx.listener(|this, _: &ShowCalendar, _, cx| this.show(Page::Calendar, cx)))
            .on_action(cx.listener(|this, _: &ShowReports, _, cx| this.show(Page::Reports, cx)))
            .on_action(cx.listener(|this, _: &PrevPeriod, w, cx| this.forward(crate::ui::date_nav::NavEvent::Prev, w, cx)))
            .on_action(cx.listener(|this, _: &NextPeriod, w, cx| this.forward(crate::ui::date_nav::NavEvent::Next, w, cx)))
            .on_action(cx.listener(|this, _: &Today, w, cx| this.forward(crate::ui::date_nav::NavEvent::Today, w, cx)))
            .on_action(cx.listener(|this, _: &ZoomIn, _, cx| this.calendar.update(cx, |c, cx| c.zoom(1, cx))))
            .on_action(cx.listener(|this, _: &ZoomOut, _, cx| this.calendar.update(cx, |c, cx| c.zoom(-1, cx))))
            .child(self.render_header(window, cx))
            .child(div().flex_1().min_h_0().flex().flex_col().child(self.render_body(cx)))
    }
}

impl TrackApp {
    fn forward(&mut self, ev: crate::ui::date_nav::NavEvent, window: &mut Window, cx: &mut Context<Self>) {
        use crate::ui::date_nav::HasDateNav;
        match self.page {
            Page::Calendar => self.calendar.update(cx, |c, cx| c.on_nav(ev, window, cx)),
            Page::Reports => self.reports.update(cx, |r, cx| r.on_nav(ev, window, cx)),
        }
    }
}
