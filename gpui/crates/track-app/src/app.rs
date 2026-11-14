use crate::actions::*;
use crate::data::DataClient;
use track_core::model::{AppConfig, Client, Project, Settings, Tag};
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

/// Emitted after `refresh_all` has re-read everything, so pages reload what they show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateEvent {
    Reloaded,
}

pub struct AppState {
    /// How many `refresh_all` rounds have finished (tests).
    pub reloads: u64,
    pub api: DataClient,
    pub data: Option<AppData>,
    pub error: Option<String>,
    pub loading: bool,
    pub persist: PersistState,
    persist_path: PathBuf,
    /// Newest AI operation already known; None until the startup baseline is read.
    pub ops_seen: Option<i64>,
    /// The AI operations offered for undo in the main window.
    pub ai_op: Option<AiOpNotice>,
}

/// 「AI が N 件変更しました［元に戻す］」: every AI operation that arrived while the notice
/// was up (until it is closed), undone together newest first.
#[derive(Debug, Clone, PartialEq)]
pub struct AiOpNotice {
    /// Newest first.
    pub operations: Vec<track_core::ops::Operation>,
    pub error: Option<String>,
    pub busy: bool,
}

impl AiOpNotice {
    /// Items changed by all the operations.
    pub fn count(&self) -> i64 {
        self.operations.iter().map(|o| o.count).sum()
    }

    pub fn summary(&self) -> String {
        match self.operations.as_slice() {
            [] => String::new(),
            [one] => one.summary.clone(),
            [newest, rest @ ..] => format!("{} ほか{}件の操作", newest.summary, rest.len()),
        }
    }

    fn ids(&self) -> Vec<i64> {
        self.operations.iter().map(|o| o.id).collect()
    }

    /// Keeps only `ids`; None when nothing is left.
    fn keep(mut self, ids: &[i64]) -> Option<Self> {
        self.operations.retain(|o| ids.contains(&o.id));
        (!self.operations.is_empty()).then_some(self)
    }
}

impl AppState {
    pub fn new(api: DataClient) -> Self {
        Self::with_path(api, PersistState::default_path())
    }

    pub fn with_path(api: DataClient, persist_path: PathBuf) -> Self {
        Self { reloads: 0, api, data: None, error: None, loading: false, persist: PersistState::load(&persist_path), persist_path, ops_seen: None, ai_op: None }
    }

    pub fn update_persist(&mut self, f: impl FnOnce(&mut PersistState)) {
        f(&mut self.persist);
        let _ = self.persist.save(&self.persist_path);
    }

    pub fn load(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        self.error = None;
        cx.notify();
        self.fetch(cx);
    }

    async fn read_all(api: DataClient) -> Result<AppData, crate::data::DataError> {
        Ok(AppData {
            projects: api.projects().await?,
            clients: api.clients().await?,
            tags: api.tags().await?,
            settings: api.settings().await?,
            config: api.config(),
        })
    }

    /// After the settings window saved something: re-read reference data, settings and config,
    /// then tell the pages to reload (same-connection writes do not change `data_version`).
    pub fn refresh_all(&mut self, cx: &mut Context<Self>) {
        let fut = Self::read_all(self.api.clone());
        let ops = self.ops_seen.map(|seen| self.api.operations_after(seen));
        let shown = self.ai_op.as_ref().map(|n| self.api.pending_operations(n.ids()));
        cx.spawn(async move |this, cx| {
            let result = fut.await;
            let ops = match ops {
                Some(f) => f.await.ok(),
                None => None,
            };
            // Offered operations may have been undone (or pruned) from the CLI meanwhile.
            let pending = match shown {
                Some(f) => f.await.ok(),
                None => None,
            };
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(data) => this.data = Some(data),
                    Err(e) => this.error = Some(e.message),
                }
                if let Some(pending) = pending {
                    this.ai_op = this.ai_op.take().and_then(|n| n.keep(&pending));
                }
                if let Some(ops) = ops {
                    this.note_operations(ops);
                }
                this.reloads += 1;
                cx.emit(StateEvent::Reloaded);
                cx.notify();
            });
        })
        .detach();
    }

    /// Startup baseline: operations made before the app opened are not offered for undo.
    pub fn read_ops_baseline(&mut self, cx: &mut Context<Self>) {
        let fut = self.api.latest_operation_id();
        cx.spawn(async move |this, cx| {
            if let Ok(id) = fut.await {
                let _ = this.update(cx, |this, _| this.ops_seen = Some(this.ops_seen.unwrap_or(0).max(id)));
            }
        })
        .detach();
    }

    /// After a restore the DB's operations (and their ids) are the snapshot's: start over from
    /// its newest one and drop a notice about an operation that no longer applies.
    pub fn reset_ops_baseline(&mut self, cx: &mut Context<Self>) {
        self.ai_op = None;
        let fut = self.api.latest_operation_id();
        cx.spawn(async move |this, cx| {
            if let Ok(id) = fut.await {
                let _ = this.update(cx, |this, cx| {
                    this.ops_seen = Some(id);
                    cx.notify();
                });
            }
        })
        .detach();
        cx.notify();
    }

    /// New operations (newest first) join the notice that is up, or start one.
    fn note_operations(&mut self, ops: Vec<track_core::ops::Operation>) {
        if let Some(max) = ops.iter().map(|o| o.id).max() {
            self.ops_seen = Some(self.ops_seen.unwrap_or(0).max(max));
        }
        let mut fresh: Vec<_> = ops.into_iter().filter(|o| o.undone_at.is_none()).collect();
        if fresh.is_empty() {
            return;
        }
        match self.ai_op.as_mut() {
            Some(n) => {
                fresh.retain(|o| !n.operations.iter().any(|x| x.id == o.id));
                fresh.append(&mut n.operations);
                n.operations = fresh;
            }
            None => self.ai_op = Some(AiOpNotice { operations: fresh, error: None, busy: false }),
        }
    }

    /// Undoes every operation in the notice, newest first; stops at one that cannot be undone.
    pub fn undo_ai_op(&mut self, cx: &mut Context<Self>) {
        let Some(notice) = self.ai_op.as_mut().filter(|n| !n.busy) else { return };
        notice.busy = true;
        notice.error = None;
        let fut = self.api.undo_operations(notice.ids());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = fut.await;
            let _ = this.update(cx, |this, cx| {
                let (done, error) = match result {
                    Ok((done, error)) => (done, error.map(|e| e.message)),
                    Err(e) => (vec![], Some(e.message)),
                };
                this.ai_op = this.ai_op.take().and_then(|mut n| {
                    n.operations.retain(|o| !done.contains(&o.id));
                    n.busy = false;
                    n.error = error;
                    (!n.operations.is_empty()).then_some(n)
                });
                this.refresh_all(cx);
                cx.notify();
            });
        })
        .detach();
    }

    pub fn dismiss_ai_op(&mut self, cx: &mut Context<Self>) {
        self.ai_op = None;
        cx.notify();
    }

    /// Saves settings; on success everything is refreshed. The task resolves with the result.
    pub fn save_settings(&mut self, patch: track_core::model::SettingsPatch, cx: &mut Context<Self>) -> gpui::Task<crate::data::DataResult<track_core::model::Settings>> {
        let fut = self.api.update_settings(patch);
        cx.spawn(async move |this, cx| {
            let result = fut.await;
            if result.is_ok() {
                let _ = this.update(cx, |this, cx| this.refresh_all(cx));
            }
            result
        })
    }

    fn fetch(&mut self, cx: &mut Context<Self>) {
        let fut = Self::read_all(self.api.clone());
        cx.spawn(async move |this, cx| {
            let result = fut.await;
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

impl gpui::EventEmitter<StateEvent> for AppState {}

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
    /// Where 「AI 連携をインストール」 installs (tests point it at a tempdir).
    pub integrations_home: PathBuf,
    pub integrations: Option<Entity<crate::ui::integrations_dialog::IntegrationsDialog>>,
    _watcher: gpui::Task<()>,
    _backups: gpui::Task<()>,
}

/// How often the DB is checked for writes by other processes (track-cli, AI agents).
const EXTERNAL_POLL: std::time::Duration = std::time::Duration::from_secs(1);

impl TrackApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let data = DataClient::from_env();
        // Tell AI skills where the CLI is (it sits next to this binary). Kept after exit: the CLI
        // works without the app.
        if track_core::guard::dev_guard(&data.data_dir).is_ok()
            && let Some(cli) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("track-cli")))
        {
            let _ = track_core::runtime::write_runtime(&data.data_dir, &cli);
        }
        let state = cx.new(|_| AppState::new(data));
        state.update(cx, |s, cx| s.load(cx));
        // Dev/screenshots: TRACK_GPUI_SETTINGS=<calendar|reports|projects|ai|backup> opens settings too.
        if let Some(section) = std::env::var("TRACK_GPUI_SETTINGS").ok().and_then(|k| crate::settings::SettingsSection::from_key(&k)) {
            let state = state.clone();
            cx.defer(move |cx| {
                crate::settings::open_settings(state, cx);
                if let Some((_, view)) = crate::settings::settings_window(cx) {
                    view.update(cx, |v, cx| v.show(section, cx));
                }
            });
        }
        Self::with_state(state, window, cx)
    }

    pub fn with_state(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        cx.subscribe(&state, |this, _, _: &StateEvent, cx| {
            this.calendar.update(cx, |c, cx| c.reload(cx));
            this.reports.update(cx, |r, cx| r.reload_if_shown(cx));
        })
        .detach();
        let calendar = cx.new(|cx| CalendarView::new(state.clone(), window, cx));
        let reports = cx.new(|cx| ReportsView::new(state.clone(), window, cx));
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        // Dev convenience: TRACK_GPUI_PAGE=reports opens the Reports screen first.
        let page = if std::env::var("TRACK_GPUI_PAGE").as_deref() == Ok("reports") { Page::Reports } else { Page::Calendar };
        state.update(cx, |s, cx| s.read_ops_baseline(cx));
        let watcher = Self::watch_external_changes(state.clone(), cx);
        let backups = Self::auto_backup(state.read(cx).api.clone(), cx);
        Self {
            state,
            page,
            calendar,
            reports,
            focus,
            integrations_home: dirs::home_dir().unwrap_or_default(),
            integrations: None,
            _watcher: watcher,
            _backups: backups,
        }
    }

    /// The automatic backups the Tauri app's server used to take: at start and every hour, a
    /// snapshot when the newest automatic one is older than `backupIntervalHours`.
    fn auto_backup(api: DataClient, cx: &mut Context<Self>) -> gpui::Task<()> {
        cx.spawn(async move |_, cx| loop {
            if let Err(e) = api.maybe_auto_backup().await {
                eprintln!("自動バックアップに失敗しました: {}", e.message);
            }
            cx.background_executor().timer(std::time::Duration::from_secs(60 * 60)).await;
        })
    }

    /// Polls `PRAGMA data_version`, which changes only when another connection commits,
    /// and reloads reference data plus whatever each page is showing.
    /// Pages and the settings window reload through `StateEvent::Reloaded`.
    fn watch_external_changes(state: Entity<AppState>, cx: &mut Context<Self>) -> gpui::Task<()> {
        let api = state.read(cx).api.clone();
        cx.spawn(async move |_, cx| {
            // Baseline first, so a write right after startup is not mistaken for it.
            let mut last = api.data_version().await.ok();
            loop {
                cx.background_executor().timer(EXTERNAL_POLL).await;
                let Ok(v) = api.data_version().await else { continue };
                if last.is_some_and(|l| l != v) {
                    // The task belongs to TrackApp, so it stops when the window goes away.
                    cx.update(|cx| state.update(cx, |s, cx| s.refresh_all(cx)));
                }
                last = Some(v);
            }
        })
    }

    pub fn open_integrations(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use gpui_component::WindowExt as _;
        let view = cx.new(|_| crate::ui::integrations_dialog::IntegrationsDialog::new(self.integrations_home.clone()));
        self.integrations = Some(view.clone());
        window.open_dialog(cx, move |dialog, _, _| dialog.title("AI 連携をインストール（開発用）").w(px(520.)).child(view.clone()));
    }

    fn show(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        cx.notify();
    }

    fn render_header(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let tab = |id: &'static str, label: &'static str, icon: IconName, page: Page, active: bool| {
            div()
                .id(id)
                .debug_selector(move || id.into())
                .flex()
                .items_center()
                .gap(px(6.))
                .h(px(24.))
                .px_2()
                .rounded(px(5.))
                .text_xs()
                .font_weight(gpui::FontWeight::MEDIUM)
                .cursor_pointer()
                .map(|d| {
                    if active {
                        d.bg(p.bg).text_color(p.text).border_1().border_color(p.border)
                    } else {
                        d.text_color(p.text_muted).hover(move |s| s.bg(p.hover).text_color(p.text))
                    }
                })
                .child(Icon::new(icon).size_3p5())
                .child(label)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _, _, cx| this.show(page, cx)))
        };
        let fullscreen = window.is_fullscreen();
        div()
            .id("header")
            .debug_selector(|| "app-header".into())
            .flex()
            .flex_shrink_0()
            .items_center()
            .h(px(38.))
            .pr_3()
            .pl(px(if fullscreen { 12. } else { 92. }))
            .bg(p.surface)
            .border_b_1()
            .border_color(p.border)
            .on_mouse_down(MouseButton::Left, |ev, window, _| {
                if ev.click_count == 2 {
                    window.titlebar_double_click();
                } else {
                    window.start_window_move();
                }
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(tab("tab-calendar", "カレンダー", IconName::Calendar, Page::Calendar, self.page == Page::Calendar))
                    .child(tab("tab-reports", "レポート", IconName::ChartPie, Page::Reports, self.page == Page::Reports)),
            )
            .child(
                div()
                    .id("open-settings")
                    .debug_selector(|| "open-settings".into())
                    .ml_auto()
                    .p_1()
                    .rounded(px(5.))
                    .cursor_pointer()
                    .text_color(p.text_muted)
                    .hover(move |s| s.text_color(p.text).bg(p.hover))
                    .child(Icon::new(IconName::Settings).size_4())
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|this, _, _, cx| crate::settings::open_settings(this.state.clone(), cx))),
            )
    }

    fn render_body(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let p = palette(cx);
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
                .text_color(p.text_muted)
                .map(|d| match error {
                    Some(e) if !loading => d
                        .child(div().font_weight(gpui::FontWeight::SEMIBOLD).text_color(p.text).child("データを読み込めません"))
                        .child(div().text_xs().text_color(p.text_muted).child(e))
                        .child(
                            div()
                                .id("retry")
                                .px_3()
                                .py_1()
                                .rounded_md()
                                .bg(p.text)
                                .text_color(p.bg)
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
        let p = palette(cx);
        div()
            .id("track-app")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(p.bg)
            .text_color(p.text)
            .font_family(".SystemUIFont")
            .on_action(cx.listener(|this, _: &ShowCalendar, _, cx| this.show(Page::Calendar, cx)))
            .on_action(cx.listener(|this, _: &InstallIntegrations, w, cx| this.open_integrations(w, cx)))
            .on_action(cx.listener(|this, _: &OpenSettings, _, cx| crate::settings::open_settings(this.state.clone(), cx)))
            .on_action(cx.listener(|this, _: &ShowReports, _, cx| this.show(Page::Reports, cx)))
            .on_action(cx.listener(|this, _: &PrevPeriod, w, cx| this.forward(crate::ui::date_nav::NavEvent::Prev, w, cx)))
            .on_action(cx.listener(|this, _: &NextPeriod, w, cx| this.forward(crate::ui::date_nav::NavEvent::Next, w, cx)))
            .on_action(cx.listener(|this, _: &Today, w, cx| this.forward(crate::ui::date_nav::NavEvent::Today, w, cx)))
            .on_action(cx.listener(|this, _: &ZoomIn, _, cx| this.calendar.update(cx, |c, cx| c.zoom(1, cx))))
            .on_action(cx.listener(|this, _: &ZoomOut, _, cx| this.calendar.update(cx, |c, cx| c.zoom(-1, cx))))
            .child(self.render_header(window, cx))
            .child(div().flex_1().min_h_0().flex().flex_col().child(self.render_body(cx)))
            .child(crate::ui::status_bar::render(self.status_left(window, cx), self.state.read(cx).ai_op.clone(), cx))
    }
}

impl TrackApp {
    /// Page facts for the status bar.
    fn status_left(&self, window: &Window, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let p = palette(cx);
        let mono = |s: String| div().font_family(MONO).text_color(p.text).child(s);
        match self.page {
            Page::Calendar => {
                let cal = self.calendar.read(cx);
                let days = cal.days(window, cx);
                let total = cal.visible_total_minutes(&days);
                let mut items = vec![div()
                    .debug_selector(|| "status-total".into())
                    .flex()
                    .gap_1()
                    .child("合計")
                    .child(mono(format!("{}:{:02}", total / 60, total % 60)))
                    .into_any_element()];
                if let Some(label) = cal.selection_label() {
                    items.push(div().debug_selector(|| "status-selection".into()).min_w_0().truncate().child(format!("選択: {label}")).into_any_element());
                }
                items
            }
            Page::Reports => {
                // The period total is already the table's 合計 row; only the row count goes here.
                let reports = self.reports.read(cx);
                let Some(data) = reports.data.as_ref() else { return vec![] };
                let unit = match reports.group_by {
                    track_core::model::GroupBy::Client => "クライアント",
                    track_core::model::GroupBy::Project => "プロジェクト",
                    track_core::model::GroupBy::Tag => "タグ",
                };
                vec![div().debug_selector(|| "status-rows".into()).child(format!("{} {unit}", data.rows.len())).into_any_element()]
            }
        }
    }

    fn forward(&mut self, ev: crate::ui::date_nav::NavEvent, window: &mut Window, cx: &mut Context<Self>) {
        use crate::ui::date_nav::HasDateNav;
        match self.page {
            Page::Calendar => self.calendar.update(cx, |c, cx| c.on_nav(ev, window, cx)),
            Page::Reports => self.reports.update(cx, |r, cx| r.on_nav(ev, window, cx)),
        }
    }
}
