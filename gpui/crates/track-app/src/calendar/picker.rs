//! ProjectPickerPopover (create flow) and the shared project list used by the edit dialog.
use track_core::model::{Project, Tag};
use crate::data::DataClient;
use crate::core::picking::{filter_projects, locked_tag_ids, merged_tag_ids, title_suggestions};
use crate::ui::theme::*;
use gpui::{
    AnyElement, App, AppContext, Context, Entity, EntityInputHandler, EventEmitter, FocusHandle, Focusable, FontWeight, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, Render, StatefulInteractiveElement, Styled, Subscription, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_component::{Icon, IconName};
use std::rc::Rc;

pub const PICKER_WIDTH: f32 = 280.;

/// Callback for a picked project (`None` = 「プロジェクトなし」).
pub type PickFn<V> = Rc<dyn Fn(&mut V, Option<String>, &mut Window, &mut Context<V>)>;

#[derive(Debug, Clone, PartialEq)]
pub enum PickerEvent {
    Submit { project_id: Option<String>, title: String, note: String, tag_ids: Vec<String> },
    Cancel,
}

#[derive(Clone)]
struct Picked {
    id: Option<String>,
    name: String,
    color: String,
    locked: Vec<String>,
}

pub struct ProjectPicker {
    projects: Vec<Project>,
    tags: Vec<Tag>,
    api: DataClient,
    picked: Option<Picked>,
    query: Entity<InputState>,
    title: Entity<InputState>,
    note: Entity<TextareaState>,
    selected_tags: Vec<String>,
    titles: Vec<String>,
    show_suggestions: bool,
    active: Option<usize>,
    focus: FocusHandle,
    _subs: Vec<Subscription>,
}

impl EventEmitter<PickerEvent> for ProjectPicker {}

impl ProjectPicker {
    pub fn new(projects: Vec<Project>, tags: Vec<Tag>, api: DataClient, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let query = cx.new(|cx| InputState::new(window, cx).placeholder("プロジェクトを検索"));
        let title = cx.new(|cx| InputState::new(window, cx).placeholder("タイトル（例: 定例ミーティング）"));
        let note = cx.new(|cx| TextareaState::new(window, cx).placeholder("メモ（任意）").rows(2));
        let subs = vec![
            cx.subscribe(&query, |_, _, _: &InputEvent, cx| cx.notify()),
            cx.subscribe_in(&title, window, |this, _, ev: &InputEvent, _, cx| match ev {
                InputEvent::Change => {
                    this.show_suggestions = true;
                    this.active = None;
                    cx.notify();
                }
                InputEvent::PressEnter { secondary: true, .. } => this.submit(cx),
                _ => {}
            }),
            cx.subscribe(&note, |this, _, ev: &InputEvent, cx| {
                if let InputEvent::PressEnter { secondary: true, .. } = ev {
                    this.submit(cx)
                }
            }),
        ];
        query.update(cx, |s, cx| s.focus(window, cx));
        Self {
            projects,
            tags,
            api,
            picked: None,
            query,
            title,
            note,
            selected_tags: Vec::new(),
            titles: Vec::new(),
            show_suggestions: false,
            active: None,
            focus: cx.focus_handle(),
            _subs: subs,
        }
    }

    pub fn pick(&mut self, project: Option<&Project>, window: &mut Window, cx: &mut Context<Self>) {
        self.picked = Some(match project {
            Some(p) => Picked { id: Some(p.id.clone()), name: p.name.clone(), color: p.color.clone(), locked: locked_tag_ids(Some(p)) },
            None => Picked { id: None, name: "プロジェクトなし".into(), color: "#a3a3a3".into(), locked: vec![] },
        });
        self.show_suggestions = true;
        self.active = None;
        self.titles.clear();
        let fut = self.api.entry_titles(project.map(|p| p.id.as_str()));
        cx.spawn(async move |this, cx| {
            if let Ok(t) = fut.await {
                let _ = this.update(cx, |this, cx| {
                    this.titles = t;
                    cx.notify();
                });
            }
        })
        .detach();
        self.title.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }

    #[cfg(test)]
    pub fn title_state(&self) -> Entity<InputState> {
        self.title.clone()
    }

    fn back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.picked = None;
        self.query.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }

    fn matches(&self, cx: &App) -> Vec<String> {
        title_suggestions(&self.titles, &self.title.read(cx).value())
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let Some(p) = self.picked.clone() else { return };
        let title: String = self.title.read(cx).value().trim().chars().take(100).collect();
        let note: String = self.note.read(cx).value().trim().chars().take(500).collect();
        cx.emit(PickerEvent::Submit { project_id: p.id, title, note, tag_ids: merged_tag_ids(&self.selected_tags, &p.locked) });
    }

    fn commit_title(&mut self, t: String, window: &mut Window, cx: &mut Context<Self>) {
        self.title.update(cx, |s, cx| s.set_value(t, window, cx));
        self.show_suggestions = false;
        self.active = None;
        cx.notify();
    }

    /// Runs before the text field handles the key (capture phase). Ignored during IME composition.
    fn on_title_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let composing = self.title.update(cx, |s, cx| s.marked_text_range(window, cx)).is_some();
        if composing {
            return;
        }
        let matches = self.matches(cx);
        let visible = self.show_suggestions && !matches.is_empty();
        let ks = &ev.keystroke;
        match ks.key.as_str() {
            "down" if !matches.is_empty() => {
                self.show_suggestions = true;
                self.active = Some(match self.active {
                    Some(i) if i + 1 < matches.len() => i + 1,
                    _ => 0,
                });
                cx.stop_propagation();
                cx.notify();
            }
            "up" if visible => {
                self.active = Some(match self.active {
                    Some(i) if i > 0 => i - 1,
                    _ => matches.len() - 1,
                });
                cx.stop_propagation();
                cx.notify();
            }
            "enter" if ks.modifiers.platform || ks.modifiers.control => {
                cx.stop_propagation();
                self.submit(cx);
            }
            "enter" if visible && self.active.is_some() => {
                cx.stop_propagation();
                let t = matches[self.active.unwrap().min(matches.len() - 1)].clone();
                self.commit_title(t, window, cx);
            }
            "escape" if visible => {
                cx.stop_propagation();
                self.show_suggestions = false;
                self.active = None;
                cx.notify();
            }
            _ => {}
        }
    }

    fn toggle_tag(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.picked.as_ref().is_some_and(|p| p.locked.iter().any(|l| l == id)) {
            return;
        }
        if let Some(i) = self.selected_tags.iter().position(|t| t == id) {
            self.selected_tags.remove(i);
        } else {
            self.selected_tags.push(id.to_string());
        }
        cx.notify();
    }
}

/// Client-grouped project list with a trailing 「プロジェクトなし」 row (ProjectPickerPopover / ProjectSelectPopover).
pub fn project_list<V: 'static>(projects: &[Project], query: &str, on_pick: PickFn<V>, cx: &mut Context<V>) -> AnyElement {
    let p = palette(cx);
    let groups = filter_projects(projects, query);
    let row = |id: gpui::SharedString, color: gpui::Hsla, label: String| {
        div()
            .id(id)
            .flex()
            .w_full()
            .items_center()
            .gap_2()
            .px_3()
            .py(px(6.))
            .text_sm()
            .cursor_pointer()
            .hover(|s| s.bg(p.hover))
            .child(div().size(px(10.)).flex_shrink_0().rounded_full().bg(color))
            .child(label)
    };
    div()
        .id("project-list")
        .max_h(px(320.))
        .overflow_y_scroll()
        .py_1()
        .when(groups.is_empty(), |d| d.child(div().px_3().py_4().flex().justify_center().text_xs().text_color(p.text_faint).child("プロジェクトがありません")))
        .children(groups.into_iter().map(|(client, ps)| {
            div()
                .child(div().px_3().pb_1().pt_2().text_size(px(10.)).font_weight(FontWeight::MEDIUM).text_color(p.text_faint).child(client.to_uppercase()))
                .children(ps.into_iter().map(|proj| {
                    let pid = proj.id.clone();
                    let on_pick = on_pick.clone();
                    row(format!("pick-project-{}", proj.id).into(), hex(&proj.color).into(), proj.name.clone())
                        .debug_selector({
                            let pid = pid.clone();
                            move || format!("project-{pid}")
                        })
                        .on_click(cx.listener(move |this, _, w, cx| on_pick(this, Some(pid.clone()), w, cx)))
                }))
        }))
        .child({
            let on_pick = on_pick.clone();
            row("pick-project-none".into(), p.text_faint, "プロジェクトなし".into())
                .mt_1()
                .border_t_1()
                .border_color(p.border)
                .text_color(p.text_muted)
                .debug_selector(|| "project-none".into())
                .on_click(cx.listener(move |this, _, w, cx| on_pick(this, None, w, cx)))
        })
        .into_any_element()
}

pub fn tag_chip(p: Palette, id: gpui::SharedString, tag: &Tag, active: bool, locked: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .rounded_full()
        .border_1()
        .px_2()
        .py(px(2.))
        .text_size(px(11.))
        .map(|d| {
            if active {
                d.border_color(gpui::transparent_black()).bg(hex(&tag.color)).text_color(p.bg)
            } else {
                d.border_color(p.border).bg(p.bg).text_color(p.text_muted).hover(|s| s.bg(p.surface))
            }
        })
        .map(|d| if locked { d.cursor_default() } else { d.cursor_pointer() })
        .child(tag.name.clone())
}

impl Focusable for ProjectPicker {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for ProjectPicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let shell = div()
            .id("project-picker")
            .debug_selector(|| "project-picker".into())
            .w(px(PICKER_WIDTH))
            .max_h(px(560.))
            .overflow_hidden()
            .rounded_md()
            .border_1()
            .border_color(p.border)
            .bg(p.bg)
            .shadow_lg()
            .occlude()
            .on_mouse_down_out(cx.listener(|_, _, _, cx| cx.emit(PickerEvent::Cancel)))
            .on_key_down(cx.listener(|_, ev: &KeyDownEvent, _, cx| {
                if ev.keystroke.key == "escape" {
                    cx.emit(PickerEvent::Cancel);
                }
            }));
        let Some(picked) = self.picked.clone() else {
            let q = self.query.read(cx).value().to_string();
            let projects = self.projects.clone();
            let on_pick: PickFn<Self> = Rc::new(move |this, id, w, cx| {
                let p = id.and_then(|id| this.projects.iter().find(|p| p.id == id).cloned());
                this.pick(p.as_ref(), w, cx);
            });
            return shell
                .child(div().border_b_1().border_color(p.border).child(Input::new(&self.query).appearance(false).px_3().py_2()))
                .child(project_list(&projects, &q, on_pick, cx));
        };
        let matches = self.matches(cx);
        let visible = self.show_suggestions && !matches.is_empty();
        let _ = window;
        shell
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(p.border)
                    .bg(p.surface)
                    .px_2()
                    .py(px(6.))
                    .child(
                        div()
                            .id("picker-back")
                            .rounded_sm()
                            .p_1()
                            .text_color(p.text_muted)
                            .cursor_pointer()
                            .hover(|s| s.bg(p.border).text_color(p.text))
                            .child(Icon::new(IconName::ChevronLeft).size_3p5())
                            .on_click(cx.listener(|this, _, w, cx| this.back(w, cx))),
                    )
                    .child(div().size(px(10.)).flex_shrink_0().rounded_full().bg(hex(&picked.color)))
                    .child(div().truncate().text_sm().text_color(p.text).child(picked.name.clone())),
            )
            .child(
                div()
                    .border_b_1()
                    .border_color(p.border)
                    .capture_key_down(cx.listener(|this, ev: &KeyDownEvent, w, cx| this.on_title_key(ev, w, cx)))
                    .child(Input::new(&self.title).appearance(false).px_3().py_2()),
            )
            .when(visible, |d| {
                d.child(
                    div()
                        .id("title-suggestions")
                        .max_h(px(160.))
                        .overflow_y_scroll()
                        .border_b_1()
                        .border_color(p.border)
                        .bg(p.surface)
                        .py_1()
                        .children(matches.iter().enumerate().map(|(i, t)| {
                            let t2 = t.clone();
                            div()
                                .id(("suggestion", i))
                                .px_3()
                                .py_1()
                                .text_sm()
                                .truncate()
                                .cursor_pointer()
                                .map(|d| if self.active == Some(i) { d.bg(p.border) } else { d.hover(|s| s.bg(p.hover)) })
                                .child(t.clone())
                                .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                                    if *h {
                                        this.active = Some(i);
                                        cx.notify();
                                    }
                                }))
                                .on_click(cx.listener(move |this, _, w, cx| this.commit_title(t2.clone(), w, cx)))
                        })),
                )
            })
            .when(!self.tags.is_empty(), |d| {
                d.child(
                    div()
                        .border_b_1()
                        .border_color(p.border)
                        .px_3()
                        .py_2()
                        .child(div().mb_1().text_size(px(10.)).font_weight(FontWeight::MEDIUM).text_color(p.text_faint).child("タグ"))
                        .child(div().flex().flex_wrap().gap_1().children(self.tags.iter().map(|tag| {
                            let locked = picked.locked.contains(&tag.id);
                            let active = locked || self.selected_tags.contains(&tag.id);
                            let id = tag.id.clone();
                            tag_chip(p, format!("picker-tag-{}", tag.id).into(), tag, active, locked)
                                .on_click(cx.listener(move |this, _, _, cx| this.toggle_tag(&id, cx)))
                        }))),
                )
            })
            .child(div().px_3().py_2().border_b_1().border_color(p.border).child(Textarea::new(&self.note).appearance(false)))
            .child(
                div().flex().justify_end().px_3().py_2().child(
                    div()
                        .id("picker-submit")
                        .debug_selector(|| "picker-submit".into())
                        .rounded_md()
                        .bg(p.text)
                        .px_3()
                        .py(px(6.))
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(p.bg)
                        .cursor_pointer()
                        .hover(|s| s.bg(p.text))
                        .child("作成 (⌘↵)")
                        .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
                ),
            )
    }
}
