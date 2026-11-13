//! EntryEditDialog.tsx 「予定を編集」 form, shown inside a gpui-component Dialog.
use super::picker::{project_list, tag_chip};
use track_core::model::{EntryPatch, Project, Tag, TimeEntry};
use crate::core::picking::{locked_tag_ids, merged_tag_ids};
use crate::ui::theme::*;
use chrono::{DateTime, Local, NaiveTime, TimeZone, Timelike, Utc};
use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Render, StatefulInteractiveElement, Styled, Subscription, Window, anchored, deferred, div, prelude::FluentBuilder, px,
};
use gpui_component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_component::time_field::{TimeField, TimeFieldState};
use gpui_component::{Icon, IconName};
use std::rc::Rc;

#[derive(Debug, Clone)]
pub enum EditEvent {
    Save(String, EntryPatch),
    Delete(String),
    Cancel,
}

pub struct EditForm {
    entry: TimeEntry,
    projects: Vec<Project>,
    tags: Vec<Tag>,
    title: Entity<InputState>,
    start: Entity<TimeFieldState>,
    end: Entity<TimeFieldState>,
    break_min: Entity<InputState>,
    note: Entity<TextareaState>,
    project_id: Option<String>,
    tag_ids: Vec<String>,
    select_open: bool,
    select_query: Entity<InputState>,
    focus: FocusHandle,
    _subs: Vec<Subscription>,
}

impl EventEmitter<EditEvent> for EditForm {}

/// Start and end typed in the dialog, both on the local day the entry starts. An end of 00:00
/// is that day's 24:00 (the next midnight), since an entry never spans days.
pub fn edited_span(entry_start: DateTime<Utc>, start: NaiveTime, end: NaiveTime) -> (DateTime<Utc>, DateTime<Utc>) {
    let day = entry_start.with_timezone(&Local).date_naive();
    let local = |d: chrono::NaiveDate, t: NaiveTime| {
        let t = NaiveTime::from_hms_opt(t.hour(), t.minute(), 0).unwrap();
        Local.from_local_datetime(&d.and_time(t)).earliest().unwrap().to_utc()
    };
    let end_day = if end.hour() == 0 && end.minute() == 0 { day.succ_opt().unwrap_or(day) } else { day };
    (local(day, start), local(end_day, end))
}

/// Clamp like the React onChange: non-numbers become 0, then 0..=600.
pub fn parse_break(s: &str) -> i64 {
    s.trim().parse::<f64>().ok().filter(|n| n.is_finite()).map(|n| (n.floor() as i64).clamp(0, 600)).unwrap_or(0)
}

impl EditForm {
    pub fn new(entry: TimeEntry, projects: Vec<Project>, tags: Vec<Tag>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let title = cx.new(|cx| InputState::new(window, cx).placeholder("タイトル（任意）").default_value(entry.title.clone().unwrap_or_default()));
        let t = |d: DateTime<Utc>| {
            let l = d.with_timezone(&Local);
            NaiveTime::from_hms_opt(l.hour(), l.minute(), 0).unwrap()
        };
        let (st, et) = (t(entry.start), t(entry.end));
        let start = cx.new(|cx| {
            let mut s = TimeFieldState::new(window, cx);
            s.set_time(st, window, cx);
            s
        });
        let end = cx.new(|cx| {
            let mut s = TimeFieldState::new(window, cx);
            s.set_time(et, window, cx);
            s
        });
        let break_min = cx.new(|cx| InputState::new(window, cx).default_value(entry.break_minutes.to_string()));
        let note = cx.new(|cx| TextareaState::new(window, cx).placeholder("メモ（任意）").rows(3).default_value(entry.note.clone().unwrap_or_default()));
        let select_query = cx.new(|cx| InputState::new(window, cx).placeholder("プロジェクトを検索"));
        let subs = vec![cx.subscribe(&select_query, |_, _, _: &InputEvent, cx| cx.notify())];
        title.update(cx, |s, cx| s.focus(window, cx));
        Self {
            project_id: entry.project_id.clone(),
            tag_ids: entry.tags.iter().map(|t| t.tag_id.clone()).collect(),
            entry,
            projects,
            tags,
            title,
            start,
            end,
            break_min,
            note,
            select_open: false,
            select_query,
            focus: cx.focus_handle(),
            _subs: subs,
        }
    }

    pub fn title_state(&self) -> Entity<InputState> {
        self.title.clone()
    }

    fn selected_project(&self) -> Option<&Project> {
        self.project_id.as_ref().and_then(|id| self.projects.iter().find(|p| &p.id == id))
    }

    pub fn save(&mut self, cx: &mut Context<Self>) {
        let title = self.title.read(cx).value().trim().chars().take(100).collect::<String>();
        let note = self.note.read(cx).value().trim().chars().take(500).collect::<String>();
        let locked = locked_tag_ids(self.selected_project());
        // React: [...new Set([...tagIds, ...lockedTagIds])]
        let mut tag_ids = merged_tag_ids(&locked, &self.tag_ids);
        tag_ids.sort_by_key(|id| self.tag_ids.iter().position(|t| t == id).unwrap_or(usize::MAX));
        let span = edited_span(self.entry.start, self.start.read(cx).time(), self.end.read(cx).time());
        let patch = EntryPatch {
            title: Some((!title.is_empty()).then_some(title)),
            start: Some(span.0),
            end: Some(span.1),
            project_id: Some(self.project_id.clone()),
            tag_ids: Some(tag_ids),
            note: Some((!note.is_empty()).then_some(note)),
            break_minutes: Some(parse_break(&self.break_min.read(cx).value())),
        };
        cx.emit(EditEvent::Save(self.entry.id.clone(), patch));
    }

    fn toggle_tag(&mut self, id: &str, cx: &mut Context<Self>) {
        if locked_tag_ids(self.selected_project()).iter().any(|l| l == id) {
            return;
        }
        if let Some(i) = self.tag_ids.iter().position(|t| t == id) {
            self.tag_ids.remove(i);
        } else {
            self.tag_ids.push(id.to_string());
        }
        cx.notify();
    }
}

fn label(p: Palette, text: &'static str) -> impl IntoElement {
    div().text_sm().font_weight(FontWeight::MEDIUM).text_color(p.text).child(text)
}

fn field() -> gpui::Div {
    div().flex().flex_col().gap_1()
}

fn footer_button(id: &'static str) -> gpui::Stateful<gpui::Div> {
    div().id(id).debug_selector(move || id.into()).rounded_md().px_3().py(px(6.)).text_sm().cursor_pointer()
}

impl Focusable for EditForm {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for EditForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let project = self.selected_project().cloned();
        let locked = locked_tag_ids(project.as_ref());
        let q = self.select_query.read(cx).value().to_string();
        div()
            .id("edit-form")
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .gap_4()
            .capture_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                if ev.keystroke.key == "enter" && ev.keystroke.modifiers.platform {
                    cx.stop_propagation();
                    this.save(cx);
                }
            }))
            .child(field().child(label(p, "タイトル")).child(Input::new(&self.title)))
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(field().flex_1().child(label(p, "開始")).child(TimeField::new(&self.start)))
                    .child(field().flex_1().child(label(p, "終了")).child(TimeField::new(&self.end)))
                    .child(field().w(px(96.)).child(label(p, "休憩 (分)")).child(Input::new(&self.break_min))),
            )
            .child(
                field().child(label(p, "プロジェクト")).child(
                    div()
                        .id("edit-project-button")
                        .debug_selector(|| "edit-project-button".into())
                        .flex()
                        .items_center()
                        .gap_2()
                        .rounded_md()
                        .border_1()
                        .border_color(p.border)
                        .px_3()
                        .py_2()
                        .text_sm()
                        .cursor_pointer()
                        .hover(|s| s.bg(p.surface))
                        .child(div().size(px(10.)).flex_shrink_0().rounded_full().bg(hex(project.as_ref().map(|p| p.color.as_str()).unwrap_or("#a3a3a3"))))
                        .child(
                            div()
                                .text_color(if project.is_some() { p.text } else { p.text_muted })
                                .child(project.as_ref().map(|p| format!("{} / {}", p.client.name, p.name)).unwrap_or_else(|| "プロジェクトなし".into())),
                        )
                        .child(div().ml_auto().text_color(p.text_faint).child(Icon::new(IconName::ChevronDown).size_3p5()))
                        .on_click(cx.listener(|this, _, w, cx| {
                            this.select_open = !this.select_open;
                            this.select_query.update(cx, |s, cx| s.focus(w, cx));
                            cx.notify();
                        }))
                        .when(self.select_open, |d| {
                            let on_pick: super::picker::PickFn<Self> = Rc::new(|this, id, _, cx| {
                                this.project_id = id;
                                this.select_open = false;
                                cx.notify();
                            });
                            d.relative().child(
                                div().absolute().top_full().left_0().child(deferred(
                                    anchored().snap_to_window_with_margin(px(8.)).child(
                                        div()
                                            .id("project-select")
                                            .mt_1()
                                            .w(px(280.))
                                            .rounded_md()
                                            .border_1()
                                            .border_color(p.border)
                                            .bg(p.bg)
                                            .shadow_lg()
                                            .occlude()
                                            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                                this.select_open = false;
                                                cx.notify();
                                            }))
                                            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                                                if ev.keystroke.key == "escape" {
                                                    cx.stop_propagation();
                                                    this.select_open = false;
                                                    cx.notify();
                                                }
                                            }))
                                            .child(div().border_b_1().border_color(p.border).child(Input::new(&self.select_query).appearance(false).px_3().py_2()))
                                            .child(project_list(&self.projects, &q, on_pick, cx)),
                                    ),
                                )
                                .with_priority(20)),
                            )
                        }),
                ),
            )
            .when(!self.tags.is_empty(), |d| {
                d.child(field().child(label(p, "タグ")).child(div().flex().flex_wrap().gap_2().children(self.tags.iter().map(|tag| {
                    let is_locked = locked.contains(&tag.id);
                    let active = is_locked || self.tag_ids.contains(&tag.id);
                    let id = tag.id.clone();
                    tag_chip(p, format!("edit-tag-{}", tag.id).into(), tag, active, is_locked)
                        .when(is_locked, |d| d.opacity(0.7))
                        .on_click(cx.listener(move |this, _, _, cx| this.toggle_tag(&id, cx)))
                }))))
            })
            .child(field().child(label(p, "メモ")).child(Textarea::new(&self.note)))
            .child(
                div()
                    .mt_2()
                    .flex()
                    .justify_between()
                    .child(
                        footer_button("edit-delete")
                            .border_1()
                            .border_color(gpui::rgb(0xfca5a5))
                            .text_color(p.danger)
                            .hover(|s| s.bg(p.danger_bg))
                            .child("削除")
                            .on_click(cx.listener(|this, _, _, cx| cx.emit(EditEvent::Delete(this.entry.id.clone())))),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                footer_button("edit-cancel")
                                    .border_1()
                                    .border_color(p.border)
                                    .hover(|s| s.bg(p.surface))
                                    .child("キャンセル")
                                    .on_click(cx.listener(|_, _, _, cx| cx.emit(EditEvent::Cancel))),
                            )
                            .child(
                                footer_button("edit-save")
                                    .bg(p.text)
                                    .text_color(p.bg)
                                    .hover(|s| s.bg(p.text))
                                    .child("保存")
                                    .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
                            ),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(d: u32, h: u32, m: u32) -> DateTime<Utc> {
        Local.with_ymd_and_hms(2026, 11, d, h, m, 0).unwrap().to_utc()
    }

    fn hm(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    #[test]
    fn edited_span_moves_an_end_at_midnight_back_into_the_day() {
        // 23:00–24:00 → end 23:45: same day, not the next day's 23:45.
        assert_eq!(edited_span(at(12, 23, 0), hm(23, 0), hm(23, 45)), (at(12, 23, 0), at(12, 23, 45)));
    }

    #[test]
    fn edited_span_end_at_0000_means_midnight_after_the_start() {
        // 22:00–23:00 → end 00:00 is 24:00 of the same day.
        assert_eq!(edited_span(at(12, 22, 0), hm(22, 0), hm(0, 0)), (at(12, 22, 0), at(13, 0, 0)));
        // Unchanged 23:00–24:00 stays as it is.
        assert_eq!(edited_span(at(12, 23, 0), hm(23, 0), hm(0, 0)), (at(12, 23, 0), at(13, 0, 0)));
    }

    #[test]
    fn edited_span_start_at_0000_stays_on_its_day() {
        assert_eq!(edited_span(at(12, 0, 0), hm(0, 0), hm(1, 0)), (at(12, 0, 0), at(12, 1, 0)));
        assert_eq!(edited_span(at(12, 1, 0), hm(0, 0), hm(1, 0)), (at(12, 0, 0), at(12, 1, 0)));
    }

    #[test]
    fn parse_break_clamps() {
        assert_eq!(parse_break("30"), 30);
        assert_eq!(parse_break("-5"), 0);
        assert_eq!(parse_break("9999"), 600);
        assert_eq!(parse_break("abc"), 0);
        assert_eq!(parse_break("15.9"), 15);
    }
}
