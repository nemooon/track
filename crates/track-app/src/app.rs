use crate::actions::*;
use crate::data::DataClient;
use track_core::model::{AppConfig, Client, Project, Settings, Tag};
use crate::calendar::CalendarView;
use crate::persist::PersistState;
use crate::reports::ReportsView;
use crate::ui::theme::*;
use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement, MouseButton, ParentElement,
    AnimationExt, Render, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
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

/// A seed that differs from the last (the clock's nanoseconds).
fn fresh_seed() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1)
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
    /// Every notice shown this session (deletions, failures, AI changes), for the status bar's
    /// bell; undo lives in its rows.
    pub history: NoticeHistory,
    /// The newest notice, shown in the status bar for `FLASH_FOR` (its history item's id).
    pub flash: Option<u64>,
    flash_task: Option<gpui::Task<()>>,
    /// The seed of the header's dot pattern; saving an entry changes it.
    pub header_seed: u64,
    /// The pattern the current one fades in from, and over how long.
    pub header_prev_seed: u64,
    pub header_fade: std::time::Duration,
    /// How many times the pattern has changed (tests; also keys the fade).
    pub header_shuffles: u64,
    sparkle: Option<gpui::Task<()>>,
}

/// How many patterns one sparkle goes through.
pub const SPARKLE_STEPS: u64 = 10;

/// How long step `step` of a sparkle lasts (and fades in over): 30ms, slowing by a sixth each
/// step to about 120ms at the end (about 0.65s in all).
pub fn sparkle_every(step: u64) -> std::time::Duration {
    std::time::Duration::from_secs_f32(0.030 * 1.17f32.powi(step as i32))
}

/// The status bar's notice about the user's own change. Plain saves say nothing: the calendar
/// already shows them and the header sparkles.
#[derive(Debug, Clone, PartialEq)]
pub enum StatusNote {
    /// 「1件削除しました：<題名>［元に戻す］」, which makes the same entry again.
    Deleted(Box<track_core::model::TimeEntry>),
    /// A change that could not be saved, in red until closed.
    Error(String),
}

impl StatusNote {
    /// The notice as one line of text (also its line in the history).
    pub fn text(&self) -> String {
        match self {
            StatusNote::Deleted(e) => format!("1件削除しました：{}", e.title.clone().filter(|t| !t.is_empty()).unwrap_or_else(|| "タイトルなし".into())),
            StatusNote::Error(msg) => msg.clone(),
        }
    }
}

/// How many notices the history keeps: older ones are pushed out by new ones.
pub const HISTORY_LIMIT: usize = 10;

/// How long a new notice shows in the status bar.
pub const FLASH_FOR: std::time::Duration = std::time::Duration::from_secs(5);

/// What 「元に戻す」 does for a history row.
#[derive(Debug, Clone, PartialEq)]
pub enum HistoryAction {
    /// Makes the deleted entry again.
    Restore(Box<track_core::model::TimeEntry>),
    /// Undoes this AI operation alone.
    UndoAi(i64),
}

/// Where a row's 「元に戻す」 stands.
#[derive(Debug, Clone, PartialEq)]
pub enum UndoState {
    Open,
    Busy,
    Done,
    /// Could not be undone, with the reason.
    Failed(String),
}

/// One line of the notice history.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryItem {
    pub id: u64,
    pub text: String,
    /// A failure, shown in red.
    pub error: bool,
    pub action: Option<HistoryAction>,
    pub undo: UndoState,
}

/// The notices shown while the app is open, newest first (not saved anywhere).
#[derive(Debug, Clone, Default)]
pub struct NoticeHistory {
    pub items: std::collections::VecDeque<HistoryItem>,
    next_id: u64,
}

impl NoticeHistory {
    /// Adds a notice on top and returns its id.
    #[cfg(test)]
    pub fn push(&mut self, text: String, error: bool) -> u64 {
        self.push_with(text, error, None)
    }

    pub fn push_with(&mut self, text: String, error: bool, action: Option<HistoryAction>) -> u64 {
        self.next_id += 1;
        self.items.push_front(HistoryItem { id: self.next_id, text, error, action, undo: UndoState::Open });
        self.items.truncate(HISTORY_LIMIT);
        self.next_id
    }


    pub fn get(&self, id: u64) -> Option<&HistoryItem> {
        self.items.iter().find(|i| i.id == id)
    }

    fn get_mut(&mut self, id: u64) -> Option<&mut HistoryItem> {
        self.items.iter_mut().find(|i| i.id == id)
    }

    /// AI operations still offered for undo.
    fn open_ai_ops(&self) -> Vec<i64> {
        self.items
            .iter()
            .filter(|i| i.undo != UndoState::Done)
            .filter_map(|i| match i.action {
                Some(HistoryAction::UndoAi(op)) => Some(op),
                _ => None,
            })
            .collect()
    }

    /// How many AI rows can still be undone.
    #[cfg(test)]
    pub fn undoable_ai(&self) -> usize {
        self.open_ai_ops().len()
    }

    /// Rows whose operation is no longer pending (undone or pruned elsewhere) are done.
    fn settle_ai(&mut self, asked: &[i64], pending: &[i64]) {
        for i in self.items.iter_mut() {
            if let Some(HistoryAction::UndoAi(op)) = i.action
                && asked.contains(&op)
                && !pending.contains(&op)
                && i.undo != UndoState::Busy
            {
                i.undo = UndoState::Done;
            }
        }
    }

    /// After a restore the old operation ids mean other things: stop offering them.
    fn forget_ai(&mut self) {
        for i in self.items.iter_mut() {
            if matches!(i.action, Some(HistoryAction::UndoAi(_))) {
                i.action = None;
            }
        }
    }
}

impl AppState {
    /// When data changes the header's dots sparkle: SPARKLE_STEPS new patterns, each fading in
    /// from the last, quick at first and slowing down, ending on the last. A new change restarts it.
    pub fn reshuffle_header(&mut self, cx: &mut Context<Self>) {
        self.sparkle = Some(cx.spawn(async move |this, cx| {
            for step in 0..SPARKLE_STEPS {
                let every = sparkle_every(step);
                let ok = this.update(cx, |s, cx| {
                    let next = fresh_seed();
                    s.header_prev_seed = s.header_seed;
                    s.header_seed = if next == s.header_seed { next.wrapping_add(1) } else { next };
                    s.header_fade = every;
                    s.header_shuffles += 1;
                    cx.notify();
                });
                if ok.is_err() {
                    break;
                }
                cx.background_executor().timer(every).await;
            }
        }));
    }

    pub fn new(api: DataClient) -> Self {
        Self::with_path(api, PersistState::default_path())
    }

    pub fn with_path(api: DataClient, persist_path: PathBuf) -> Self {
        let seed = fresh_seed();
        Self { reloads: 0, api, data: None, error: None, loading: false, persist: PersistState::load(&persist_path), persist_path, ops_seen: None, history: NoticeHistory::default(), flash: None, flash_task: None, header_seed: seed, header_prev_seed: seed, header_fade: std::time::Duration::ZERO, header_shuffles: 0, sparkle: None }
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
        let asked = self.history.open_ai_ops();
        let shown = (!asked.is_empty()).then(|| self.api.pending_operations(asked.clone()));
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
                    this.history.settle_ai(&asked, &pending);
                }
                if let Some(ops) = ops {
                    this.note_operations(ops, cx);
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
        self.history.forget_ai();
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

    /// New AI operations (newest first) each join the history, the newest flashing.
    fn note_operations(&mut self, ops: Vec<track_core::ops::Operation>, cx: &mut Context<Self>) {
        if let Some(max) = ops.iter().map(|o| o.id).max() {
            self.ops_seen = Some(self.ops_seen.unwrap_or(0).max(max));
        }
        let known = self.history.open_ai_ops();
        for op in ops.iter().rev().filter(|o| o.undone_at.is_none() && !known.contains(&o.id)) {
            let text = format!("AI が {} 件変更しました：{}", op.count, op.summary);
            self.notice(text, false, Some(HistoryAction::UndoAi(op.id)), cx);
        }
    }

    /// Adds a notice to the history and shows it in the status bar for `FLASH_FOR`.
    fn notice(&mut self, text: String, error: bool, action: Option<HistoryAction>, cx: &mut Context<Self>) {
        let id = self.history.push_with(text, error, action);
        self.flash = Some(id);
        self.flash_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(FLASH_FOR).await;
            let _ = this.update(cx, |this, cx| {
                if this.flash == Some(id) {
                    this.flash = None;
                    cx.notify();
                }
            });
        }));
        cx.notify();
    }

    /// A change of the user's own: a deletion (undoable from the history) or a failure.
    pub fn set_note(&mut self, note: StatusNote, cx: &mut Context<Self>) {
        let text = note.text();
        match note {
            StatusNote::Deleted(entry) => self.notice(text, false, Some(HistoryAction::Restore(entry)), cx),
            StatusNote::Error(_) => self.notice(text, true, None, cx),
        }
    }

    /// 「履歴を消す」.
    pub fn clear_history(&mut self, cx: &mut Context<Self>) {
        self.history.items.clear();
        self.flash = None;
        cx.notify();
    }

    /// 「元に戻す」 on a history row. A deletion's entry is handed back for the calendar to make
    /// again (the row is then done); an AI operation is undone here.
    pub fn undo_from_history(&mut self, id: u64, cx: &mut Context<Self>) -> Option<track_core::model::TimeEntry> {
        let item = self.history.get_mut(id).filter(|i| matches!(i.undo, UndoState::Open | UndoState::Failed(_)))?;
        match item.action.clone()? {
            HistoryAction::Restore(entry) => {
                item.undo = UndoState::Done;
                cx.notify();
                Some(*entry)
            }
            HistoryAction::UndoAi(op) => {
                item.undo = UndoState::Busy;
                let fut = self.api.undo_operations(vec![op]);
                cx.notify();
                cx.spawn(async move |this, cx| {
                    let result = fut.await;
                    let _ = this.update(cx, |this, cx| {
                        let state = match result {
                            Ok((done, _)) if done.contains(&op) => UndoState::Done,
                            Ok((_, error)) => UndoState::Failed(error.map(|e| e.message).unwrap_or_else(|| "取り消せませんでした".into())),
                            Err(e) => UndoState::Failed(e.message),
                        };
                        if let Some(item) = this.history.get_mut(id) {
                            item.undo = state;
                        }
                        this.refresh_all(cx);
                        cx.notify();
                    });
                })
                .detach();
                None
            }
        }
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
    /// The notice history list is open above the status bar's bell.
    pub history_open: bool,
    /// The header's 「…」 menu is open.
    pub app_menu_open: bool,
    /// Where the bell is, so a press on it is left to the bell (which toggles) instead of
    /// closing the list as a press outside it.
    pub bell_bounds: std::rc::Rc<std::cell::Cell<gpui::Bounds<gpui::Pixels>>>,
    _watcher: gpui::Task<()>,
    _backups: gpui::Task<()>,
    _new_day: gpui::Task<()>,
    _changes: gpui::Task<()>,
}

/// The header's icons (tabs and settings): large enough for the day in the calendar icon.
pub const HEADER_ICON: gpui::Pixels = px(18.);

/// How long from `now` until just after the next local midnight (when the calendar tab's day
/// changes).
pub fn until_next_day(now: chrono::NaiveDateTime) -> std::time::Duration {
    let next = (now.date() + chrono::Duration::days(1)).and_hms_opt(0, 0, 1).unwrap();
    (next - now).to_std().unwrap_or_default()
}

/// The calendar icon with today's day in its body (Lucide `calendar` leaves the body empty).
fn calendar_day_icon(day: u32, side: f32) -> gpui::AnyElement {
    let unit = side / 24.;
    div()
        .debug_selector(|| "tab-calendar-icon".into())
        .relative()
        .size(px(side))
        .flex_none()
        .child(Icon::new(IconName::Calendar).size(px(side)))
        .child(
            div()
                .debug_selector(move || format!("tab-calendar-day-{day}"))
                .absolute()
                .left(px(3. * unit))
                .right(px(3. * unit))
                .top(px(9. * unit))
                .bottom(px(3. * unit))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(8.))
                .line_height(px(8.))
                .font_weight(gpui::FontWeight::BOLD)
                .child(day.to_string()),
        )
        .into_any_element()
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
        // macOS switched light/dark: follow it when the setting is 自動.
        cx.observe_window_appearance(window, |this, window, cx| this.on_system_appearance(is_dark(window.appearance()), cx)).detach();
        let watcher = Self::watch_external_changes(state.clone(), cx);
        let changes = Self::sparkle_on_changes(state.clone(), cx);
        let backups = Self::auto_backup(state.read(cx).api.clone(), cx);
        // Redraw just after midnight so the calendar tab shows the new day.
        let new_day = cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(until_next_day(chrono::Local::now().naive_local())).await;
            if this.update(cx, |_, cx| cx.notify()).is_err() {
                break;
            }
        });
        Self {
            state,
            page,
            calendar,
            reports,
            focus,
            integrations_home: dirs::home_dir().unwrap_or_default(),
            integrations: None,
            history_open: false,
            app_menu_open: false,
            bell_bounds: Default::default(),
            _watcher: watcher,
            _backups: backups,
            _new_day: new_day,
            _changes: changes,
        }
    }

    /// Sparkles the header's dots whenever an operation changes data (a burst of writes is one
    /// sparkle).
    fn sparkle_on_changes(state: Entity<AppState>, cx: &mut Context<Self>) -> gpui::Task<()> {
        let rx = state.read(cx).api.changes();
        cx.spawn(async move |_, cx| {
            while rx.recv().await.is_ok() {
                while rx.try_recv().is_ok() {}
                cx.update(|cx| state.update(cx, |s, cx| s.reshuffle_header(cx)));
            }
        })
    }

    /// Automatic backups: at start and every hour, a
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
                    // Changes from the CLI or an AI agent sparkle like the app's own.
                    cx.update(|cx| state.update(cx, |s, cx| {
                        s.refresh_all(cx);
                        s.reshuffle_header(cx);
                    }));
                }
                last = Some(v);
            }
        })
    }

    pub fn open_integrations(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use gpui_component::WindowExt as _;
        let view = cx.new(|_| crate::ui::integrations_dialog::IntegrationsDialog::new(self.integrations_home.clone()));
        self.integrations = Some(view.clone());
        window.open_dialog(cx, move |dialog, _, _| dialog.title(if track_core::guard::GUARDED { "AI 連携をインストール（開発用）" } else { "AI 連携をインストール" }).w(px(520.)).child(view.clone()));
    }

    fn show(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        cx.notify();
    }

    fn render_header(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let tab = |id: &'static str, label: &'static str, icon: gpui::AnyElement, page: Page, active: bool| {
            div()
                .id(id)
                .debug_selector(move || id.into())
                .flex()
                .items_center()
                .gap(px(6.))
                .h_full()
                .px(px(2.))
                .text_size(px(13.))
                .font_weight(gpui::FontWeight::MEDIUM)
                .cursor_pointer()
                // An underline on the header's edge marks the page. Every tab has it (clear when
                // inactive), so switching changes no size.
                .border_b_2()
                .border_color(gpui::transparent_black())
                // The same 2px on top, so the label stays on the header's midline.
                .pt(px(2.))
                .map(|d| {
                    if active {
                        d.text_color(p.text).border_color(p.text)
                    } else {
                        d.text_color(p.text_muted).hover(move |s| s.text_color(p.text))
                    }
                })
                .child(icon)
                .child(div().debug_selector(move || format!("{id}-label")).child(label))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _, _, cx| this.show(page, cx)))
        };
        let fullscreen = window.is_fullscreen();
        let (from, seed, fade, shuffles) = {
            let s = self.state.read(cx);
            (s.header_prev_seed, s.header_seed, s.header_fade, s.header_shuffles)
        };
        let dot_color = p.text;
        let dots = move |t: f32| {
            gpui::canvas(|_, _, _| (), move |bounds, _, window, _| crate::ui::header_dots::paint(bounds, (from, seed, t), dot_color, window)).size_full()
        };
        div()
            .id("header")
            .debug_selector(|| "app-header".into())
            .relative()
            .flex()
            .flex_shrink_0()
            .items_center()
            .h(APP_HEADER_HEIGHT)
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
            // The dot pattern behind everything (it takes no clicks). A new pattern fades in
            // from the last over `fade`.
            .child({
                let layer = div().absolute().top_0().left_0().size_full();
                if fade.is_zero() || from == seed {
                    layer.child(dots(1.)).into_any_element()
                } else {
                    layer
                        .with_animation(gpui::SharedString::from(format!("header-dots-{shuffles}")), gpui::Animation::new(fade), move |d, t| d.child(dots(t)))
                        .into_any_element()
                }
            })
            .child(
                // Page tabs, not a toolbar group: navigation looks different from view settings.
                div()
                    .flex()
                    .h_full()
                    .gap(px(14.))
                    .child(tab("tab-calendar", "カレンダー", calendar_day_icon(chrono::Datelike::day(&chrono::Local::now()), f32::from(HEADER_ICON)), Page::Calendar, self.page == Page::Calendar))
                    .child(tab("tab-reports", "レポート", div().debug_selector(|| "tab-reports-icon".into()).size(HEADER_ICON).child(crate::ui::icons::clipboard_list().size(HEADER_ICON)).into_any_element(), Page::Reports, self.page == Page::Reports)),
            )
            .child(crate::ui::app_menu::render(self, cx))
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
                                .bg(p.primary)
                                .text_color(p.on_color)
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
            .on_action(cx.listener(|_, _: &ShowAbout, _, cx| crate::ui::about::open_about(cx)))
            .on_action(cx.listener(|this, _: &ShowReports, _, cx| this.show(Page::Reports, cx)))
            .on_action(cx.listener(|this, _: &PrevPeriod, w, cx| this.forward(crate::ui::date_nav::NavEvent::Prev, w, cx)))
            .on_action(cx.listener(|this, _: &NextPeriod, w, cx| this.forward(crate::ui::date_nav::NavEvent::Next, w, cx)))
            .on_action(cx.listener(|this, _: &Today, w, cx| this.forward(crate::ui::date_nav::NavEvent::Today, w, cx)))
            .on_action(cx.listener(|this, _: &ZoomIn, _, cx| this.calendar.update(cx, |c, cx| c.zoom(1, cx))))
            .on_action(cx.listener(|this, _: &ZoomOut, _, cx| this.calendar.update(cx, |c, cx| c.zoom(-1, cx))))
            .child(self.render_header(window, cx))
            .child(div().flex_1().min_h_0().flex().flex_col().child(self.render_body(cx)))
            .child({
                let left = self.status_left(window, cx);
                crate::ui::status_bar::render(self, left, cx)
            })
    }
}

impl TrackApp {
    /// The platform appearance changed (or tests simulate it): re-resolve with the saved setting.
    pub fn on_system_appearance(&mut self, system_dark: bool, cx: &mut Context<Self>) {
        let setting = self.state.read(cx).api.full_config().appearance;
        apply_setting(setting, system_dark, cx);
    }

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
                    .child(mono(cal.duration(total, cx)))
                    .into_any_element()];
                if let Some(label) = cal.selection_label() {
                    items.push(div().debug_selector(|| "status-selection".into()).min_w_0().truncate().child(format!("選択: {label}")).into_any_element());
                }
                items
            }
            Page::Reports => {
                // The period total is already the table's 合計 row; only the row count goes here.
                let reports = self.reports.read(cx);
                let Some(count) = reports.row_count() else { return vec![] };
                let unit = match reports.group_by {
                    track_core::model::GroupBy::Client => "クライアント",
                    track_core::model::GroupBy::Project => "プロジェクト",
                    track_core::model::GroupBy::Tag => "タグ",
                };
                vec![div().debug_selector(|| "status-rows".into()).child(format!("{count} {unit}")).into_any_element()]
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
