//! EntryEditDialog.tsx 「予定を編集」 form, shown inside a gpui-component Dialog.
use super::picker::{project_list, tag_chip};
use crate::api::{EntryPatch, Project, Tag, TimeEntry};
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

/// `applyTime`: keep the date of `orig` (local), replace hour/minute.
pub fn apply_time(orig: DateTime<Utc>, t: NaiveTime) -> DateTime<Utc> {
    let local = orig.with_timezone(&Local).date_naive().and_time(NaiveTime::from_hms_opt(t.hour(), t.minute(), 0).unwrap());
    Local.from_local_datetime(&local).earliest().unwrap().to_utc()
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
        let patch = EntryPatch {
            title: Some((!title.is_empty()).then_some(title)),
            start: Some(apply_time(self.entry.start, self.start.read(cx).time())),
            end: Some(apply_time(self.entry.end, self.end.read(cx).time())),
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

fn label(text: &'static str) -> impl IntoElement {
    div().text_sm().font_weight(FontWeight::MEDIUM).text_color(n700()).child(text)
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
            .child(field().child(label("タイトル")).child(Input::new(&self.title)))
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(field().flex_1().child(label("開始")).child(TimeField::new(&self.start)))
                    .child(field().flex_1().child(label("終了")).child(TimeField::new(&self.end)))
                    .child(field().w(px(96.)).child(label("休憩 (分)")).child(Input::new(&self.break_min))),
            )
            .child(
                field().child(label("プロジェクト")).child(
                    div()
                        .id("edit-project-button")
                        .debug_selector(|| "edit-project-button".into())
                        .flex()
                        .items_center()
                        .gap_2()
                        .rounded_md()
                        .border_1()
                        .border_color(n300())
                        .px_3()
                        .py_2()
                        .text_sm()
                        .cursor_pointer()
                        .hover(|s| s.bg(n50()))
                        .child(div().size(px(10.)).flex_shrink_0().rounded_full().bg(hex(project.as_ref().map(|p| p.color.as_str()).unwrap_or("#a3a3a3"))))
                        .child(
                            div()
                                .text_color(if project.is_some() { n900() } else { n500() })
                                .child(project.as_ref().map(|p| format!("{} / {}", p.client.name, p.name)).unwrap_or_else(|| "プロジェクトなし".into())),
                        )
                        .child(div().ml_auto().text_color(n400()).child(Icon::new(IconName::ChevronDown).size_3p5()))
                        .on_click(cx.listener(|this, _, w, cx| {
                            this.select_open = !this.select_open;
                            this.select_query.update(cx, |s, cx| s.focus(w, cx));
                            cx.notify();
                        }))
                        .when(self.select_open, |d| {
                            let on_pick: Rc<dyn Fn(&mut Self, Option<String>, &mut Window, &mut Context<Self>)> = Rc::new(|this, id, _, cx| {
                                this.project_id = id;
                                this.select_open = false;
                                cx.notify();
                            });
                            d.child(
                                deferred(
                                    anchored().snap_to_window_with_margin(px(8.)).child(
                                        div()
                                            .id("project-select")
                                            .mt_1()
                                            .w(px(280.))
                                            .rounded_md()
                                            .border_1()
                                            .border_color(n200())
                                            .bg(white())
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
                                            .child(div().border_b_1().border_color(n200()).child(Input::new(&self.select_query).appearance(false).px_3().py_2()))
                                            .child(project_list(&self.projects, &q, on_pick, cx)),
                                    ),
                                )
                                .with_priority(20),
                            )
                        }),
                ),
            )
            .when(!self.tags.is_empty(), |d| {
                d.child(field().child(label("タグ")).child(div().flex().flex_wrap().gap_2().children(self.tags.iter().map(|tag| {
                    let is_locked = locked.contains(&tag.id);
                    let active = is_locked || self.tag_ids.contains(&tag.id);
                    let id = tag.id.clone();
                    tag_chip(format!("edit-tag-{}", tag.id).into(), tag, active, is_locked)
                        .when(is_locked, |d| d.opacity(0.7))
                        .on_click(cx.listener(move |this, _, _, cx| this.toggle_tag(&id, cx)))
                }))))
            })
            .child(field().child(label("メモ")).child(Textarea::new(&self.note)))
            .child(
                div()
                    .mt_2()
                    .flex()
                    .justify_between()
                    .child(
                        footer_button("edit-delete")
                            .border_1()
                            .border_color(gpui::rgb(0xfca5a5))
                            .text_color(red600())
                            .hover(|s| s.bg(red50()))
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
                                    .border_color(n300())
                                    .hover(|s| s.bg(n50()))
                                    .child("キャンセル")
                                    .on_click(cx.listener(|_, _, _, cx| cx.emit(EditEvent::Cancel))),
                            )
                            .child(
                                footer_button("edit-save")
                                    .bg(n900())
                                    .text_color(white())
                                    .hover(|s| s.bg(n700()))
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

    #[test]
    fn apply_time_keeps_original_date() {
        let orig = Local.with_ymd_and_hms(2026, 11, 13, 0, 0, 0).unwrap().to_utc();
        let t = apply_time(orig, NaiveTime::from_hms_opt(9, 45, 0).unwrap());
        assert_eq!(t, Local.with_ymd_and_hms(2026, 11, 13, 9, 45, 0).unwrap().to_utc());
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
