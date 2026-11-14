pub mod custom_output;
pub mod weekly_report;

use track_core::model::{GroupBy, ReportEntry, ReportQuery, ReportRange, ReportResponse};
use crate::app::AppState;
use crate::core::geometry::at_minutes;
use track_core::report::group::group_entries_by_title;
use track_core::report::view::*;
use crate::core::time::format_duration;
use crate::persist::DurationStyle;
use crate::ui::{controls, date_nav::{self, DateNav, HasDateNav, NavEvent, NavKind}, theme::*};
use chrono::{Datelike, Duration, Local, NaiveDate};
use gpui::{
    AnyElement, AppContext, Context, Entity, FontWeight, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    Render, StatefulInteractiveElement, Styled, Subscription, Task, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{Icon, IconName};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterKind {
    Client,
    Project,
    Tag,
}

#[cfg(test)]
pub type CustomOutputView = custom_output::CustomOutput;

#[derive(Debug, Clone)]
pub enum Children {
    Loading,
    Failed(String),
    Loaded(Vec<ReportEntry>),
}

pub struct ReportsView {
    pub state: Entity<AppState>,
    pub anchor: NaiveDate,
    pub range: ReportRange,
    pub group_by: GroupBy,
    pub client_ids: Vec<String>,
    pub project_ids: Vec<String>,
    pub tag_ids: Vec<String>,
    nav: DateNav,
    pub data: Option<ReportResponse>,
    pub loading: bool,
    pub error: Option<String>,
    requested: Option<ReportQuery>,
    load_task: Option<Task<()>>,
    pub children: HashMap<String, Children>,
    pub expanded: HashSet<String>,
    expand_all: bool,
    pub open_filter: Option<FilterKind>,
    filter_query: Entity<InputState>,
    pub output: Option<Entity<custom_output::CustomOutput>>,
    pub weekly: Option<Entity<weekly_report::WeeklyReport>>,
    totals_copied: bool,
    copy_task: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl ReportsView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let filter_query = cx.new(|cx| InputState::new(window, cx));
        let subs = vec![cx.subscribe(&filter_query, |_, _, _: &InputEvent, cx| cx.notify())];
        let expanded = state.read(cx).persist.reports_expanded.get("project").cloned().unwrap_or_default().into_iter().collect();
        Self {
            state,
            anchor: Local::now().date_naive(),
            range: ReportRange::Week,
            group_by: GroupBy::Project,
            client_ids: vec![],
            project_ids: vec![],
            tag_ids: vec![],
            nav: DateNav::default(),
            data: None,
            loading: false,
            error: None,
            requested: None,
            load_task: None,
            children: HashMap::new(),
            expanded,
            expand_all: false,
            open_filter: None,
            filter_query,
            output: None,
            weekly: None,
            totals_copied: false,
            copy_task: None,
            _subs: subs,
        }
    }

    pub fn current_query(&self) -> ReportQuery {
        ReportQuery {
            range: self.range,
            anchor: at_minutes(self.anchor, 0).to_utc(),
            group_by: self.group_by,
            client_ids: self.client_ids.clone(),
            project_ids: self.project_ids.clone(),
            tag_ids: self.tag_ids.clone(),
        }
    }

    /// Re-fetch `/reports` and drop loaded children (they are fetched again when shown).
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let q = self.current_query();
        self.requested = Some(q.clone());
        self.loading = self.data.is_none();
        let api = self.state.read(cx).api.clone();
        let fut = api.report(&q);
        self.load_task = Some(cx.spawn(async move |this, cx| {
            let result = fut.await;
            let _ = this.update(cx, |this, cx| {
                if this.requested.as_ref() != Some(&q) {
                    return;
                }
                this.loading = false;
                match result {
                    Ok(data) => {
                        if this.expand_all {
                            for r in &data.rows {
                                this.expanded.insert(row_path(this.group_by, &r.key));
                            }
                        }
                        this.data = Some(data);
                        this.error = None;
                    }
                    Err(e) => this.error = Some(e.message),
                }
                this.children.clear();
                this.load_visible_children(cx);
                cx.notify();
            });
        }));
    }

    /// Live-update hook: only refetch once the screen has loaded something.
    pub fn reload_if_shown(&mut self, cx: &mut Context<Self>) {
        if self.requested.is_some() {
            self.reload(cx);
        }
    }

    fn ensure_loaded(&mut self, cx: &mut Context<Self>) {
        if self.state.read(cx).data.is_none() {
            return;
        }
        if self.requested.as_ref() != Some(&self.current_query()) {
            self.data = None;
            self.reload(cx);
        }
    }

    fn load_visible_children(&mut self, cx: &mut Context<Self>) {
        let Some(data) = self.data.clone() else { return };
        for r in &data.rows {
            let path = row_path(self.group_by, &r.key);
            if self.expanded.contains(&path) && !self.children.contains_key(&path) {
                self.load_children(path, r.key.clone(), cx);
            }
        }
    }

    fn load_children(&mut self, path: String, key: String, cx: &mut Context<Self>) {
        self.children.insert(path.clone(), Children::Loading);
        let q = child_query(&self.current_query(), self.group_by, &key);
        let fut = self.state.read(cx).api.report_entries(&q);
        cx.spawn(async move |this, cx| {
            let result = fut.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(Children::Loading) = this.children.get(&path) {
                    let next = match result {
                        Ok(r) => Children::Loaded(r.entries),
                        Err(e) => Children::Failed(e.message),
                    };
                    this.children.insert(path, next);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn save_expanded(&mut self, cx: &mut Context<Self>) {
        let key = group_key(self.group_by).to_string();
        let list: Vec<String> = self.expanded.iter().cloned().collect();
        self.state.update(cx, |s, _| s.update_persist(|p| {
            p.reports_expanded.insert(key, list);
        }));
    }

    pub fn toggle_row(&mut self, path: String, key: String, cx: &mut Context<Self>) {
        if !self.expanded.remove(&path) {
            self.expanded.insert(path.clone());
            if !self.children.contains_key(&path) {
                self.load_children(path, key, cx);
            }
        }
        self.save_expanded(cx);
        cx.notify();
    }

    pub fn toggle_all(&mut self, cx: &mut Context<Self>) {
        if !self.expanded.is_empty() {
            self.expanded.clear();
            self.expand_all = false;
        } else {
            for r in self.data.as_ref().map(|d| d.rows.clone()).unwrap_or_default() {
                self.expanded.insert(row_path(self.group_by, &r.key));
            }
            self.expand_all = true;
            self.load_visible_children(cx);
        }
        self.save_expanded(cx);
        cx.notify();
    }

    fn style(&self, cx: &gpui::App) -> DurationStyle {
        self.state.read(cx).persist.duration_style
    }

    /// A duration as the report shows it ("1.5h" or "1h 30m").
    #[cfg(test)]
    pub fn duration(&self, minutes: i64, cx: &gpui::App) -> String {
        format_duration(minutes, self.style(cx))
    }

    fn set_duration_style(&mut self, style: DurationStyle, cx: &mut Context<Self>) {
        self.state.update(cx, |s, _| s.update_persist(|p| p.duration_style = style));
        cx.notify();
    }

    /// Copies "project<TAB>hours" lines for the shown week, for pasting into a timesheet.
    pub fn copy_weekly_totals(&mut self, cx: &mut Context<Self>) {
        let fut = self.state.read(cx).api.weekly_copy(&self.current_query());
        self.copy_task = Some(cx.spawn(async move |this, cx| {
            let result = fut.await;
            let ok = this
                .update(cx, |this, cx| {
                    match result {
                        Ok(text) => {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                            this.totals_copied = true;
                            this.error = None;
                        }
                        Err(e) => this.error = Some(e.message),
                    }
                    cx.notify();
                    this.totals_copied
                })
                .unwrap_or(false);
            if ok {
                cx.background_executor().timer(std::time::Duration::from_secs(2)).await;
                let _ = this.update(cx, |this, cx| {
                    this.totals_copied = false;
                    cx.notify();
                });
            }
        }));
    }

    pub fn set_group_by(&mut self, g: GroupBy, cx: &mut Context<Self>) {
        if self.group_by == g {
            return;
        }
        self.group_by = g;
        self.expanded = self.state.read(cx).persist.reports_expanded.get(group_key(g)).cloned().unwrap_or_default().into_iter().collect();
        self.expand_all = false;
        cx.notify();
    }

    fn options(&self, kind: FilterKind, cx: &gpui::App) -> Vec<FilterOption> {
        let Some(data) = self.state.read(cx).data.as_ref() else { return vec![] };
        match kind {
            FilterKind::Client => data.clients.iter().filter(|c| !c.archived).map(|c| FilterOption { id: c.id.clone(), label: c.name.clone(), hint: None, color: None }).collect(),
            FilterKind::Project => data
                .projects
                .iter()
                .filter(|p| !p.archived && (self.client_ids.is_empty() || self.client_ids.contains(&p.client_id)))
                .map(|p| FilterOption { id: p.id.clone(), label: p.name.clone(), hint: Some(p.client.name.clone()), color: Some(p.color.clone()) })
                .collect(),
            FilterKind::Tag => data.tags.iter().map(|t| FilterOption { id: t.id.clone(), label: t.name.clone(), hint: None, color: Some(t.color.clone()) }).collect(),
        }
    }

    fn selected(&self, kind: FilterKind) -> &Vec<String> {
        match kind {
            FilterKind::Client => &self.client_ids,
            FilterKind::Project => &self.project_ids,
            FilterKind::Tag => &self.tag_ids,
        }
    }

    pub fn set_selected(&mut self, kind: FilterKind, next: Vec<String>, cx: &mut Context<Self>) {
        match kind {
            FilterKind::Client => {
                let projects = self.state.read(cx).data.as_ref().map(|d| d.projects.clone()).unwrap_or_default();
                self.project_ids = keep_projects_for_clients(&self.project_ids, &next, &projects);
                self.client_ids = next;
            }
            FilterKind::Project => self.project_ids = next,
            FilterKind::Tag => self.tag_ids = next,
        }
        cx.notify();
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let kind = if self.range == ReportRange::Week { NavKind::Week } else { NavKind::Month };
        controls::toolbar(&p)
            .child(date_nav::render(self.nav, self.anchor, kind, cx))
            .child(controls::group(&p).children([(ReportRange::Week, "週"), (ReportRange::Month, "月")].into_iter().map(|(r, label)| {
                controls::button(&p, label, self.range == r, false).child(label).on_click(cx.listener(move |this, _, _, cx| {
                    this.range = r;
                    cx.notify();
                }))
            })))
            .child(controls::group(&p).children([(GroupBy::Client, "クライアント"), (GroupBy::Project, "プロジェクト"), (GroupBy::Tag, "タグ")].into_iter().map(
                |(g, label)| controls::button(&p, label, self.group_by == g, false).child(label).on_click(cx.listener(move |this, _, _, cx| this.set_group_by(g, cx))),
            )))
            .child(controls::group(&p).children([(DurationStyle::Decimal, "duration-decimal", "1.5h"), (DurationStyle::HoursMinutes, "duration-hm", "1h 30m")].into_iter().map(
                |(s, id, label)| {
                    controls::button(&p, id, self.style(cx) == s, false)
                        .debug_selector(move || id.into())
                        .child(label)
                        .on_click(cx.listener(move |this, _, _, cx| this.set_duration_style(s, cx)))
                },
            )))
    }

    fn render_filter(&self, kind: FilterKind, label: &'static str, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let options = self.options(kind, cx);
        let selected = self.selected(kind).clone();
        let active = !selected.is_empty();
        let open = self.open_filter == Some(kind);
        let id = format!("filter-{label}");
        div()
            .relative()
            .child(
                div()
                    .id(gpui::SharedString::from(id.clone()))
                    .debug_selector(move || id.clone())
                    .flex()
                    .h(px(32.))
                    .items_center()
                    .gap(px(6.))
                    .rounded_md()
                    .border_1()
                    .px_3()
                    .text_xs()
                    .cursor_pointer()
                    .map(|d| if active { d.border_color(p.text).bg(p.text).text_color(p.bg) } else { d.border_color(p.border).bg(p.bg).text_color(p.text).hover(|s| s.bg(p.surface)) })
                    .child(filter_label(label, &options, &selected))
                    .child(div().opacity(0.7).child(Icon::new(IconName::ChevronDown).size_3()))
                    .on_click(cx.listener(move |this, _, w, cx| {
                        this.open_filter = if this.open_filter == Some(kind) { None } else { Some(kind) };
                        this.filter_query.update(cx, |s, cx| {
                            s.set_value("", w, cx);
                            s.set_placeholder(format!("{label}を検索"), w, cx);
                            s.focus(w, cx);
                        });
                        cx.notify();
                    })),
            )
            .when(open, |d| {
                let q = self.filter_query.read(cx).value().to_string();
                let rows: Vec<AnyElement> = filter_options(&options, &q)
                    .into_iter()
                    .map(|o| {
                        let checked = selected.contains(&o.id);
                        let oid = o.id.clone();
                        let sel = format!("filter-option-{}", o.id);
                        div()
                            .id(gpui::SharedString::from(format!("opt-{}", o.id)))
                            .debug_selector(move || sel.clone())
                            .flex()
                            .w_full()
                            .items_center()
                            .gap_2()
                            .px_3()
                            .py(px(6.))
                            .text_sm()
                            .cursor_pointer()
                            .hover(|s| s.bg(p.hover))
                            .child(
                                div()
                                    .size_4()
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_sm()
                                    .border_1()
                                    .map(|d| if checked { d.border_color(p.text).bg(p.text).text_color(p.bg) } else { d.border_color(p.border).bg(p.bg) })
                                    .when(checked, |d| d.child(Icon::new(IconName::Check).size_3())),
                            )
                            .when_some(o.color.clone(), |d, c| d.child(div().size(px(10.)).flex_shrink_0().rounded_full().bg(hex(&c))))
                            .child(div().flex_1().truncate().child(o.label.clone()))
                            .when_some(o.hint.clone(), |d, h| d.child(div().text_size(px(10.)).text_color(p.text_faint).child(h)))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let mut next = this.selected(kind).clone();
                                if let Some(i) = next.iter().position(|x| *x == oid) {
                                    next.remove(i);
                                } else {
                                    next.push(oid.clone());
                                }
                                this.set_selected(kind, next, cx);
                            }))
                            .into_any_element()
                    })
                    .collect();
                let empty = rows.is_empty();
                d.child(date_nav::below(
                    div()
                        .id("filter-popover")
                        .mt_1()
                        .w(px(240.))
                        .overflow_hidden()
                        .rounded_md()
                        .border_1()
                        .border_color(p.border)
                        .bg(p.bg)
                        .shadow_lg()
                        .occlude()
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                            this.open_filter = None;
                            cx.notify();
                        }))
                        .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                            if ev.keystroke.key == "escape" {
                                this.open_filter = None;
                                cx.notify();
                            }
                        }))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .border_b_1()
                                .border_color(p.border)
                                .px_3()
                                .py_1()
                                .child(div().flex_1().child(Input::new(&self.filter_query).appearance(false)))
                                .when(active, |d| {
                                    d.child(
                                        div()
                                            .id("filter-clear")
                                            .ml_2()
                                            .text_size(px(11.))
                                            .text_color(p.text_muted)
                                            .cursor_pointer()
                                            .hover(|s| s.text_color(p.text))
                                            .child("クリア")
                                            .on_click(cx.listener(move |this, _, _, cx| this.set_selected(kind, vec![], cx))),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .id("filter-options")
                                .max_h(px(300.))
                                .overflow_y_scroll()
                                .py_1()
                                .when(empty, |d| d.child(div().px_3().py_4().flex().justify_center().text_xs().text_color(p.text_faint).child(format!("該当する{label}がありません"))))
                                .children(rows),
                        ),
                ))
            })
    }

    fn render_filter_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let any = !(self.client_ids.is_empty() && self.project_ids.is_empty() && self.tag_ids.is_empty());
        div()
            .flex()
            .flex_shrink_0()
            .flex_wrap()
            .items_center()
            .gap_2()
            .border_b_1()
            .border_color(p.border)
            .bg(with_alpha(p.surface, 0.6))
            .px_3()
            .py_2()
            .child(div().mr_1().text_xs().font_weight(FontWeight::MEDIUM).text_color(p.text_muted).child("絞り込み"))
            .child(self.render_filter(FilterKind::Client, "クライアント", cx))
            .child(self.render_filter(FilterKind::Project, "プロジェクト", cx))
            .child(self.render_filter(FilterKind::Tag, "タグ", cx))
            .when(any, |d| {
                d.child(
                    div()
                        .id("filter-clear-all")
                        .ml_1()
                        .text_xs()
                        .text_color(p.text_muted)
                        .cursor_pointer()
                        .hover(|s| s.text_color(p.text).underline())
                        .child("すべてクリア")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.client_ids.clear();
                            this.project_ids.clear();
                            this.tag_ids.clear();
                            cx.notify();
                        })),
                )
            })
    }

    fn render_details(&self, data: &ReportResponse, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let group_same = self.state.read(cx).persist.group_same_titles;
        let style = self.style(cx);
        let total = data.total_minutes;
        let cols = |content: AnyElement, date: AnyElement, tags: AnyElement, time: AnyElement, pct: AnyElement| {
            div()
                .flex()
                .items_center()
                .child(div().flex_1().min_w_0().px_3().child(content))
                .child(div().w(px(160.)).flex_shrink_0().px_3().child(date))
                .child(div().w(px(112.)).flex_shrink_0().px_3().child(tags))
                .child(div().w(px(96.)).flex_shrink_0().px_3().flex().justify_end().child(time))
                .child(div().w(px(168.)).flex_shrink_0().px_3().flex().justify_end().child(pct))
        };
        let e = || div().into_any_element();
        let header = div()
            .flex_shrink_0()
            .border_b_1()
            .border_color(p.border)
            .bg(with_alpha(p.surface, 0.8))
            .py_2()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .text_color(p.text_muted)
            .child(cols(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .id("toggle-all")
                            .debug_selector(|| "toggle-all".into())
                            .size_4()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_sm()
                            .text_color(p.text_faint)
                            .cursor_pointer()
                            .hover(|s| s.bg(with_alpha(p.border, 0.7)).text_color(p.text_muted))
                            .child(Icon::new(IconName::ChevronsUpDown).size_3())
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_all(cx))),
                    )
                    .child("内容")
                    .child(
                        div()
                            .id("group-same-titles")
                            .debug_selector(|| "group-same-titles".into())
                            .ml_2()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_xs()
                            .font_weight(FontWeight::NORMAL)
                            .text_color(p.text_muted)
                            .cursor_pointer()
                            .child("まとめる")
                            .child(
                                div()
                                    .relative()
                                    .h_4()
                                    .w(px(28.))
                                    .flex_shrink_0()
                                    .rounded_full()
                                    .bg(if group_same { p.text } else { p.text_faint })
                                    .child(div().absolute().top(px(2.)).left(px(if group_same { 14. } else { 2. })).size_3().rounded_full().bg(p.bg).shadow_sm()),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.state.update(cx, |s, _| s.update_persist(|p| p.group_same_titles = !p.group_same_titles));
                                cx.notify();
                            })),
                    )
                    .into_any_element(),
                div().child("日時").into_any_element(),
                div().child("タグ").into_any_element(),
                div().child("時間").into_any_element(),
                div().child("割合").into_any_element(),
            ));
        let tag_pills = |tags: &[track_core::model::Tag]| -> AnyElement {
            if tags.is_empty() {
                div().text_color(p.text_faint).child("—").into_any_element()
            } else {
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .children(tags.iter().map(|t| div().rounded_full().px(px(6.)).text_size(px(10.)).text_color(p.bg).bg(hex(&t.color)).child(t.name.clone())))
                    .into_any_element()
            }
        };
        let mut body: Vec<AnyElement> = Vec::new();
        for (i, r) in data.rows.iter().enumerate() {
            let path = row_path(self.group_by, &r.key);
            let open = self.expanded.contains(&path);
            let (p2, k2) = (path.clone(), r.key.clone());
            let sel = format!("row-{}", r.key);
            body.push(
                div()
                    .id(gpui::SharedString::from(format!("row-{path}")))
                    .debug_selector(move || sel.clone())
                    .border_b_1()
                    .border_color(p.hover)
                    .py_2()
                    .text_sm()
                    .cursor_pointer()
                    .hover(|s| s.bg(p.surface))
                    .child(cols(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().size_4().flex_shrink_0().flex().items_center().justify_center().text_color(p.text_faint).child(Icon::new(if open { IconName::ChevronDown } else { IconName::ChevronRight }).size_3()))
                            .child(div().size_3().flex_shrink_0().rounded_sm().bg(hex(&row_color(r.color.as_deref(), i))))
                            .child(div().min_w_0().truncate().child(r.label.clone()))
                            .into_any_element(),
                        e(),
                        e(),
                        div().font_family(MONO).child(format_duration(r.total_minutes, style)).into_any_element(),
                        share_cell(p, &r.key, &row_color(r.color.as_deref(), i), r.total_minutes, total),
                    ))
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_row(p2.clone(), k2.clone(), cx)))
                    .into_any_element(),
            );
            if !open {
                continue;
            }
            let child_row = |content: AnyElement, date: AnyElement, tags: AnyElement, minutes: i64| {
                div()
                    .border_b_1()
                    .border_color(p.hover)
                    .py(px(6.))
                    .text_sm()
                    .text_color(p.text)
                    .child(cols(
                        div().pl(px(20.)).flex().items_center().gap_2().child(div().size_4().flex_shrink_0()).child(content).into_any_element(),
                        date,
                        tags,
                        div().font_family(MONO).child(format_duration(minutes, style)).into_any_element(),
                        div().text_color(p.text_faint).child(format!("{}%", percent(minutes, r.total_minutes))).into_any_element(),
                    ))
                    .into_any_element()
            };
            let note_row = |text: &'static str| div().py_2().pl(px(32.)).px_3().text_xs().text_color(p.text_faint).child(text).into_any_element();
            match self.children.get(&path) {
                None | Some(Children::Loading) => body.push(note_row("読み込み中…")),
                Some(Children::Failed(msg)) => body.push(div().py_2().pl(px(32.)).px_3().text_xs().text_color(p.danger).child(format!("読み込みに失敗しました（{msg}）")).into_any_element()),
                Some(Children::Loaded(es)) if es.is_empty() => body.push(note_row("エントリがありません")),
                Some(Children::Loaded(es)) if group_same => {
                    for g in group_entries_by_title(es) {
                        body.push(child_row(
                            div()
                                .flex()
                                .min_w_0()
                                .items_center()
                                .gap_2()
                                .child(div().size_2().flex_shrink_0().rounded_full().bg(g.project.as_ref().map(|pr| gpui::Hsla::from(hex(&pr.color))).unwrap_or(p.text_faint)))
                                .child(div().min_w_0().truncate().map(|d| match &g.title {
                                    Some(t) if !t.is_empty() => d.child(t.clone()),
                                    _ => d.text_color(p.text_faint).child("（タイトルなし）"),
                                }))
                                .child(div().text_size(px(10.)).text_color(p.text_faint).child(format!("×{}", g.count)))
                                .into_any_element(),
                            div().text_xs().text_color(p.text_faint).child("—").into_any_element(),
                            tag_pills(&g.tags),
                            g.minutes,
                        ));
                    }
                }
                Some(Children::Loaded(es)) => {
                    for en in es {
                        let s = en.start.with_timezone(&Local);
                        let end = en.end.with_timezone(&Local);
                        body.push(child_row(
                            div()
                                .flex()
                                .min_w_0()
                                .items_center()
                                .gap_2()
                                .child(div().size_2().flex_shrink_0().rounded_full().bg(en.project.as_ref().map(|pr| gpui::Hsla::from(hex(&pr.color))).unwrap_or(p.text_faint)))
                                .child(div().min_w_0().truncate().map(|d| match &en.title {
                                    Some(t) if !t.is_empty() => d.child(t.clone()),
                                    _ => d.text_color(p.text_faint).child("（タイトルなし）"),
                                }))
                                .into_any_element(),
                            div()
                                .text_xs()
                                .text_color(p.text_muted)
                                .whitespace_nowrap()
                                .child(format!("{} ({}) {}–{}", s.format("%m/%d"), crate::core::time::weekday_ja(s.date_naive()), s.format("%H:%M"), end.format("%H:%M")))
                                .into_any_element(),
                            tag_pills(&en.tags),
                            en.minutes,
                        ));
                    }
                }
            }
        }
        div()
            .flex()
            .flex_col()
            .min_h_0()
            .min_w_0()
            .flex_1()
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .min_h(px(48.))
                    .flex_shrink_0()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .border_b_1()
                    .border_color(p.border)
                    .px_3()
                    .py(px(10.))
                    .child(div().text_sm().font_weight(FontWeight::MEDIUM).text_color(p.text).child("明細"))
                    .child(self.render_output_buttons(cx)),
            )
            .child(header)
            .child(div().id("report-body").flex_1().min_h_0().overflow_y_scroll().children(body))
            .child(
                div().flex_shrink_0().border_t_1().border_color(p.border).bg(with_alpha(p.surface, 0.5)).py_2().text_sm().font_weight(FontWeight::MEDIUM).child(cols(
                    div().child("合計").into_any_element(),
                    e(),
                    e(),
                    div().font_family(MONO).child(format_duration(total, style)).into_any_element(),
                    div().child("100%").into_any_element(),
                )),
            )
            .into_any_element()
    }

    fn render_output_buttons(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let small = |id: &'static str| div().id(id).debug_selector(move || id.into()).flex().items_center().gap(px(6.)).rounded_md().border_1().px(px(10.)).py_1().text_xs().font_weight(FontWeight::MEDIUM).cursor_pointer();
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_end()
            .gap_3()
            .when(self.range == ReportRange::Week, |d| {
                d.child(
                    small("weekly-copy-totals")
                        .border_color(p.border)
                        .bg(p.bg)
                        .text_color(p.text)
                        .hover(|s| s.bg(p.surface))
                        .child(Icon::new(if self.totals_copied { IconName::Check } else { IconName::Copy }).size_3p5())
                        .child(if self.totals_copied { "コピーしました" } else { "週の合計をコピー" })
                        .on_click(cx.listener(|this, _, _, cx| this.copy_weekly_totals(cx))),
                )
                .child(
                    small("weekly-open")
                        .border_color(gpui::rgb(0xa7f3d0))
                        .bg(gpui::rgb(0xecfdf5))
                        .text_color(gpui::rgb(0x065f46))
                        .hover(|s| s.bg(gpui::rgb(0xd1fae5)))
                        .child(Icon::new(IconName::Bot).size_3p5())
                        .child("週報を生成")
                        .on_click(cx.listener(|this, _, w, cx| this.open_weekly(w, cx))),
                )
            })
            .child(
                small("output-open")
                    .border_color(gpui::rgb(0xbfdbfe))
                    .bg(gpui::rgb(0xeff6ff))
                    .text_color(gpui::rgb(0x1e40af))
                    .hover(|s| s.bg(gpui::rgb(0xdbeafe)))
                    .child(Icon::new(IconName::FileText).size_3p5())
                    .child("出力を作成")
                    .on_click(cx.listener(|this, _, w, cx| this.open_output(w, cx))),
            )
            .into_any_element()
    }

    pub fn open_output(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use gpui_component::WindowExt as _;
        let ctx = custom_output::OutputContext {
            query: self.current_query(),
            anchor: self.anchor,
            rows: self.data.as_ref().map(|d| d.rows.clone()).unwrap_or_default(),
            total: self.data.as_ref().map(|d| d.total_minutes).unwrap_or(0),
        };
        let state = self.state.clone();
        let view = cx.new(|cx| custom_output::CustomOutput::new(state, ctx, cx));
        self.output = Some(view.clone());
        window.open_dialog(cx, move |dialog, _, _| dialog.title("カスタム出力").w(px(1100.)).child(view.clone()));
    }

    pub fn open_weekly(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use gpui_component::WindowExt as _;
        let (state, q, anchor) = (self.state.clone(), self.current_query(), self.anchor);
        let view = cx.new(|cx| weekly_report::WeeklyReport::new(state, q, anchor, window, cx));
        self.weekly = Some(view.clone());
        window.open_dialog(cx, move |dialog, _, _| dialog.title("週報").w(px(760.)).child(view.clone()));
    }
}

impl HasDateNav for ReportsView {
    fn date_nav(&mut self) -> &mut DateNav {
        &mut self.nav
    }

    fn on_nav(&mut self, ev: NavEvent, _window: &mut Window, cx: &mut Context<Self>) {
        self.anchor = match (ev, self.range) {
            (NavEvent::Prev, ReportRange::Week) => self.anchor - Duration::days(7),
            (NavEvent::Next, ReportRange::Week) => self.anchor + Duration::days(7),
            (NavEvent::Prev, ReportRange::Month) => shift_month(self.anchor, -1),
            (NavEvent::Next, ReportRange::Month) => shift_month(self.anchor, 1),
            (NavEvent::Today, _) => Local::now().date_naive(),
            (NavEvent::Pick(d), _) => d,
        };
        cx.notify();
    }
}

pub fn shift_month(d: NaiveDate, delta: i32) -> NaiveDate {
    let total = d.year() * 12 + d.month0() as i32 + delta;
    let (y, m) = (total.div_euclid(12), total.rem_euclid(12) as u32 + 1);
    let last = crate::core::time::month_range(NaiveDate::from_ymd_opt(y, m, 1).unwrap()).1 - Duration::days(1);
    NaiveDate::from_ymd_opt(y, m, d.day().min(last.day())).unwrap()
}

impl Render for ReportsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        self.ensure_loaded(cx);
        let error_banner = self.error.clone().map(|msg| controls::error_banner(&p, "reports-retry", &msg).on_click(cx.listener(|this, _, _, cx| this.reload(cx))));
        let body: AnyElement = match (&self.data, self.loading) {
            (None, false) if self.error.is_some() => div().flex_1().into_any_element(),
            (None, _) | (_, true) => div().flex_1().flex().items_center().justify_center().text_sm().text_color(p.text_faint).child("読み込み中…").into_any_element(),
            (Some(d), _) if d.rows.is_empty() => div().flex_1().flex().items_center().justify_center().text_color(p.text_faint).child("この期間にデータがありません").into_any_element(),
            (Some(d), _) => {
                let d = d.clone();
                self.render_details(&d, cx)
            }
        };
        div()
            .id("reports-root")
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(self.render_toolbar(cx))
            .child(div().flex_1().min_h_0().flex().flex_col().bg(p.bg).child(self.render_filter_bar(cx)).children(error_banner).child(body))
    }
}

/// Share of the period: a thin bar in the row's colour plus the percentage (replaces the donut).
fn share_cell(p: Palette, key: &str, color: &str, minutes: i64, total: i64) -> AnyElement {
    let ratio = if total > 0 { (minutes as f32 / total as f32).clamp(0., 1.) } else { 0. };
    let (bar, fill) = (format!("share-bar-{key}"), format!("share-fill-{key}"));
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .debug_selector(move || bar.clone())
                .w(px(96.))
                .h(px(6.))
                .flex_shrink_0()
                .rounded_full()
                .bg(p.border_subtle)
                .overflow_hidden()
                .child(div().debug_selector(move || fill.clone()).h_full().w(gpui::relative(ratio)).rounded_full().bg(hex(color))),
        )
        .child(div().w(px(36.)).flex().justify_end().font_family(MONO).text_color(p.text_muted).child(format!("{}%", percent(minutes, total))))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shift_month_clamps_day() {
        let d = |y, m, day| NaiveDate::from_ymd_opt(y, m, day).unwrap();
        assert_eq!(shift_month(d(2026, 3, 31), -1), d(2026, 2, 28));
        assert_eq!(shift_month(d(2026, 12, 15), 1), d(2027, 1, 15));
    }
}
