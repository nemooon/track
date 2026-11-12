//! 「プロジェクト」: clients, projects and tags (ProjectsPage.tsx). Like the Tauri screen, clients and
//! projects are archived rather than deleted; only tags can be deleted. Add/edit happens in an
//! inline form at the top of the section instead of a modal.
use super::form;
use crate::app::AppState;
use crate::data::{DataClient, DataResult};
use crate::ui::theme::*;
use gpui::{
    AnyElement, AppContext, Context, Div, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::input::{Input, InputState};
use gpui_component::{Icon, IconName};
use std::future::Future;
use track_core::model::{Client, ClientPatch, Project, ProjectPatch, Tag, TagPatch};

/// `PROJECT_COLORS` in src/client/lib/utils.ts (shared with track-cli).
pub use track_core::edit::PROJECT_COLORS as COLORS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditKind {
    Client,
    Project,
    Tag,
}

/// What the editor opens with.
struct Draft {
    kind: EditKind,
    id: Option<String>,
    name: String,
    client_id: String,
    color: String,
    tag_ids: Vec<String>,
}

impl Draft {
    fn new(kind: EditKind) -> Self {
        Draft { kind, id: None, name: String::new(), client_id: String::new(), color: String::new(), tag_ids: vec![] }
    }
}

pub struct Editor {
    pub kind: EditKind,
    /// `None` when adding.
    pub id: Option<String>,
    pub name: Entity<InputState>,
    pub client_id: String,
    pub color: String,
    pub tag_ids: Vec<String>,
    pub error: Option<String>,
}

pub struct ProjectSettings {
    state: Entity<AppState>,
    pub clients: Vec<Client>,
    pub projects: Vec<Project>,
    pub tags: Vec<Tag>,
    pub editor: Option<Editor>,
    deleting_tag: Option<Tag>,
    error: Option<String>,
}

fn api(state: &Entity<AppState>, cx: &gpui::App) -> DataClient {
    state.read(cx).api.clone()
}

fn random_color() -> String {
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0) as usize;
    COLORS[n % COLORS.len()].to_string()
}

impl ProjectSettings {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        // Restores and outside writes (CLI, AI) re-read everything; follow them.
        cx.subscribe(&state, |this, _, _: &crate::app::StateEvent, cx| this.reload(cx)).detach();
        let mut s = ProjectSettings { state, clients: vec![], projects: vec![], tags: vec![], editor: None, deleting_tag: None, error: None };
        s.reload(cx);
        s
    }

    /// Re-reads the lists (archived included).
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let api = api(&self.state, cx);
        let (c, p, t) = (api.clients(), api.projects_all(), api.tags());
        cx.spawn(async move |this, cx| {
            let result = async { Ok::<_, crate::data::DataError>((c.await?, p.await?, t.await?)) }.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok((mut c, mut p, t)) => {
                        // Archived rows go last, like the Tauri screen.
                        c.sort_by_key(|x| x.archived);
                        p.sort_by_key(|x| x.archived);
                        (this.clients, this.projects, this.tags) = (c, p, t);
                        // An editor for something that no longer exists (e.g. after a restore) closes.
                        let gone = this.editor.as_ref().and_then(|e| e.id.as_ref().map(|id| (e.kind, id.clone()))).is_some_and(|(kind, id)| match kind {
                            EditKind::Client => !this.clients.iter().any(|x| x.id == id),
                            EditKind::Project => !this.projects.iter().any(|x| x.id == id),
                            EditKind::Tag => !this.tags.iter().any(|x| x.id == id),
                        });
                        if gone {
                            this.editor = None;
                        }
                    }
                    Err(e) => this.error = Some(e.message),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Runs a write; on success closes the editor, reloads and refreshes the main window.
    /// On failure the message is shown in the editor (or above the lists).
    fn write<T: Send + 'static>(&mut self, fut: impl Future<Output = DataResult<T>> + Send + 'static, close_editor: bool, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let result = fut.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(_) => {
                        if close_editor {
                            this.editor = None;
                        }
                        this.error = None;
                        this.reload(cx);
                        this.state.update(cx, |s, cx| s.refresh_all(cx));
                    }
                    Err(e) => {
                        let message = match e.code.as_str() {
                            "tag_already_exists" => "同じ名前のタグが既にあります".to_string(),
                            "invalid_input" => "名前を入力してください（100文字以内、タグは50文字以内）".to_string(),
                            _ => e.message,
                        };
                        match this.editor.as_mut() {
                            Some(ed) if close_editor => ed.error = Some(message),
                            _ => this.error = Some(message),
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn open_editor(&mut self, d: Draft, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("名前").default_value(d.name));
        cx.observe(&input, |_, _, cx| cx.notify()).detach();
        input.update(cx, |i, cx| i.focus(window, cx));
        self.editor = Some(Editor { kind: d.kind, id: d.id, name: input, client_id: d.client_id, color: d.color, tag_ids: d.tag_ids, error: None });
        cx.notify();
    }

    fn save_editor(&mut self, cx: &mut Context<Self>) {
        let Some(ed) = &self.editor else { return };
        let name = ed.name.read(cx).value().trim().to_string();
        if name.is_empty() {
            if let Some(ed) = self.editor.as_mut() {
                ed.error = Some("名前を入力してください".into());
            }
            cx.notify();
            return;
        }
        let api = api(&self.state, cx);
        let (id, client_id, color, tag_ids) = (ed.id.clone(), ed.client_id.clone(), ed.color.clone(), ed.tag_ids.clone());
        match (ed.kind, id) {
            (EditKind::Client, None) => self.write(api.create_client(name), true, cx),
            (EditKind::Client, Some(id)) => self.write(api.update_client(id, ClientPatch { name: Some(name), ..Default::default() }), true, cx),
            (EditKind::Project, None) => {
                if client_id.is_empty() {
                    return;
                }
                self.write(api.create_project(client_id, name, color, tag_ids), true, cx)
            }
            (EditKind::Project, Some(id)) => self.write(
                api.update_project(id, ProjectPatch { name: Some(name), color: Some(color), client_id: Some(client_id), tag_ids: Some(tag_ids), ..Default::default() }),
                true,
                cx,
            ),
            (EditKind::Tag, None) => self.write(api.create_tag(name, color), true, cx),
            (EditKind::Tag, Some(id)) => self.write(api.update_tag(id, TagPatch { name: Some(name), color: Some(color) }), true, cx),
        }
    }

    fn render_editor(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let ed = self.editor.as_ref()?;
        let title = match (ed.kind, ed.id.is_some()) {
            (EditKind::Client, false) => "クライアント追加",
            (EditKind::Client, true) => "クライアント編集",
            (EditKind::Project, false) => "プロジェクト追加",
            (EditKind::Project, true) => "プロジェクト編集",
            (EditKind::Tag, false) => "タグ追加",
            (EditKind::Tag, true) => "タグ編集",
        };
        let colors = div().flex().flex_wrap().gap_2().children(COLORS.iter().enumerate().map(|(i, c)| {
            let selected = ed.color == *c;
            let id = format!("editor-color-{i}");
            let sel = id.clone();
            let color = c.to_string();
            div()
                .id(SharedString::from(id))
                .debug_selector(move || sel.clone())
                .size(px(24.))
                .rounded_full()
                .cursor_pointer()
                .bg(hex(c))
                .when(selected, |d| d.border_2().border_color(n900()))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(ed) = this.editor.as_mut() {
                        ed.color = color.clone();
                    }
                    cx.notify();
                }))
        }));
        let chip = |id: String, label: String, on: bool| {
            let sel = id.clone();
            div()
                .id(SharedString::from(id))
                .debug_selector(move || sel.clone())
                .px_2()
                .py_1()
                .rounded_md()
                .text_xs()
                .cursor_pointer()
                .when(on, |d| d.bg(n900()).text_color(white()))
                .when(!on, |d| d.border_1().border_color(n200()).text_color(n700()))
                .child(label)
        };
        let mut body = div().flex().flex_col().gap_3().p_4().child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(title));
        if ed.kind == EditKind::Project {
            let current = ed.client_id.clone();
            body = body.child(form_line(
                "クライアント",
                div().flex().flex_wrap().gap_2().children(
                    self.clients.iter().filter(|c| !c.archived || c.id == current).map(|c| {
                        let cid = c.id.clone();
                        chip(format!("editor-client-{}", c.id), c.name.clone(), c.id == current).on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(ed) = this.editor.as_mut() {
                                ed.client_id = cid.clone();
                            }
                            cx.notify();
                        }))
                    }),
                ),
            ));
        }
        body = body.child(form_line("名前", div().w(px(320.)).child(Input::new(&ed.name))));
        if ed.kind != EditKind::Client {
            body = body.child(form_line("カラー", colors));
        }
        if ed.kind == EditKind::Project && !self.tags.is_empty() {
            body = body.child(form_line(
                "タグ",
                div().flex().flex_wrap().gap_2().children(self.tags.iter().map(|t| {
                    let tid = t.id.clone();
                    chip(format!("editor-tag-{}", t.id), t.name.clone(), ed.tag_ids.contains(&t.id)).on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(ed) = this.editor.as_mut() {
                            if let Some(p) = ed.tag_ids.iter().position(|x| *x == tid) {
                                ed.tag_ids.remove(p);
                            } else {
                                ed.tag_ids.push(tid.clone());
                            }
                        }
                        cx.notify();
                    }))
                })),
            ));
        }
        let can_save = !ed.name.read(cx).value().trim().is_empty() && (ed.kind != EditKind::Project || !ed.client_id.is_empty());
        body = body.child(
            div()
                .flex()
                .items_center()
                .justify_end()
                .gap_2()
                .when_some(ed.error.clone(), |d, e| d.child(div().debug_selector(|| "editor-error".into()).mr_auto().text_xs().text_color(red700()).child(e)))
                .child(form::button("editor-cancel", "キャンセル", false, true).on_click(cx.listener(|this, _, _, cx| {
                    this.editor = None;
                    cx.notify();
                })))
                .child(
                    form::button("editor-save", if ed.id.is_some() { "更新" } else { "作成" }, true, can_save)
                        .on_click(cx.listener(|this, _, _, cx| this.save_editor(cx))),
                ),
        );
        Some(div().mb_5().rounded_lg().border_1().border_color(n300()).bg(n50()).child(body).into_any_element())
    }
}

fn form_line(label: &str, control: impl IntoElement) -> Div {
    div().flex().items_center().gap_4().child(div().w(px(90.)).flex_shrink_0().text_xs().text_color(n600()).child(label.to_string())).child(control)
}

fn icon_button(id: String, icon: IconName, title: &'static str, enabled: bool) -> gpui::Stateful<Div> {
    let sel = id.clone();
    div()
        .id(SharedString::from(id))
        .debug_selector(move || sel.clone())
        .p_1()
        .rounded_md()
        .text_color(n500())
        .when(enabled, |d| d.cursor_pointer().hover(|s| s.bg(n100()).text_color(n900())))
        .when(!enabled, |d| d.opacity(0.4))
        .child(Icon::new(icon).size_4())
        .tooltip(move |window, cx| gpui_component::tooltip::Tooltip::new(title).build(window, cx))
}

/// The section title with its 「＋ 追加」 button; `on_add` runs when the button is clicked.
fn section_header(title: &str, add: gpui::Stateful<Div>) -> Div {
    div().flex().items_center().justify_between().mb_2().child(div().text_base().font_weight(FontWeight::SEMIBOLD).child(title.to_string())).child(add)
}

fn list() -> Div {
    div().flex().flex_col().mb_8().rounded_md().border_1().border_color(n200())
}

fn list_row(id: String, archived: bool) -> gpui::Stateful<Div> {
    let sel = id.clone();
    div()
        .id(SharedString::from(id))
        .debug_selector(move || sel.clone())
        .flex()
        .items_center()
        .gap_3()
        .px_4()
        .py_2()
        .border_b_1()
        .border_color(n100())
        .text_sm()
        .when(archived, |d| d.bg(n50()).text_color(n500()))
}

fn archived_badge(id: String) -> Div {
    let sel = id.clone();
    div().debug_selector(move || sel.clone()).px_2().rounded_md().bg(n200()).text_xs().text_color(n600()).child("アーカイブ済み")
}

impl Render for ProjectSettings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_client = self.clients.iter().find(|c| !c.archived).map(|c| c.id.clone());
        let mut root = div().children(self.render_editor(cx));
        if let Some(e) = self.error.clone() {
            root = root.child(div().mb_4().text_xs().text_color(red700()).child(e));
        }

        // Clients
        let clients = list().children(self.clients.iter().map(|c| {
            let (edit_c, arch_c) = (c.clone(), c.clone());
            list_row(format!("client-row-{}", c.id), c.archived)
                .child(div().flex_1().child(c.name.clone()))
                .when(c.archived, |d| d.child(archived_badge(format!("client-archived-{}", c.id))))
                .child(icon_button(format!("client-edit-{}", c.id), IconName::Settings, "編集", true).on_click(cx.listener(move |this, _, w, cx| {
                    this.open_editor(Draft { id: Some(edit_c.id.clone()), name: edit_c.name.clone(), ..Draft::new(EditKind::Client) }, w, cx)
                })))
                .child(
                    icon_button(format!("client-archive-{}", c.id), if c.archived { IconName::Undo } else { IconName::Inbox }, if c.archived { "復元" } else { "アーカイブ" }, true)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let api = api(&this.state, cx);
                            this.write(api.update_client(arch_c.id.clone(), ClientPatch { archived: Some(!arch_c.archived), ..Default::default() }), false, cx)
                        })),
                )
        }));
        root = root
            .child(section_header(
                "クライアント",
                form::button("client-add", "＋ 追加", true, true).on_click(cx.listener(|this, _, w, cx| {
                    this.open_editor(Draft::new(EditKind::Client), w, cx)
                })),
            ))
            .child(if self.clients.is_empty() { div().mb_8().text_sm().text_color(n400()).child("クライアントがありません") } else { clients });

        // Projects
        let projects = list().children(self.projects.iter().map(|p| {
            let client_archived = self.clients.iter().any(|c| c.id == p.client_id && c.archived);
            let can_toggle = !(p.archived && client_archived);
            let (edit_p, arch_p) = (p.clone(), p.clone());
            list_row(format!("project-row-{}", p.id), p.archived)
                .child(div().size(px(10.)).rounded_full().bg(hex(&p.color)))
                .child(div().flex_1().min_w_0().truncate().child(p.name.clone()))
                .child(div().text_xs().text_color(n500()).child(p.client.name.clone()))
                .when(p.archived, |d| d.child(archived_badge(format!("project-archived-{}", p.id))))
                .child(icon_button(format!("project-edit-{}", p.id), IconName::Settings, "編集", true).on_click(cx.listener(move |this, _, w, cx| {
                    let p = &edit_p;
                    this.open_editor(Draft { kind: EditKind::Project, id: Some(p.id.clone()), name: p.name.clone(), client_id: p.client_id.clone(), color: p.color.clone(), tag_ids: p.tags.iter().map(|t| t.tag_id.clone()).collect() }, w, cx)
                })))
                .child(
                    icon_button(
                        format!("project-archive-{}", p.id),
                        if p.archived { IconName::Undo } else { IconName::Inbox },
                        if !can_toggle { "クライアントを先に復元してください" } else if p.archived { "復元" } else { "アーカイブ" },
                        can_toggle,
                    )
                    .when(can_toggle, |d| {
                        d.on_click(cx.listener(move |this, _, _, cx| {
                            let api = api(&this.state, cx);
                            this.write(api.update_project(arch_p.id.clone(), ProjectPatch { archived: Some(!arch_p.archived), ..Default::default() }), false, cx)
                        }))
                    }),
                )
        }));
        root = root
            .child(section_header(
                "プロジェクト",
                form::button("project-add", "＋ 追加", true, active_client.is_some()).when_some(active_client, |d, first| {
                    d.on_click(cx.listener(move |this, _, w, cx| this.open_editor(Draft { client_id: first.clone(), color: random_color(), ..Draft::new(EditKind::Project) }, w, cx)))
                }),
            ))
            .child(if self.projects.is_empty() { div().mb_8().text_sm().text_color(n400()).child("プロジェクトがありません") } else { projects });

        // Tags
        let tags = list().children(self.tags.iter().map(|t| {
            let (edit_t, del_t) = (t.clone(), t.clone());
            list_row(format!("tag-row-{}", t.id), false)
                .child(div().size(px(10.)).rounded_full().bg(hex(&t.color)))
                .child(div().flex_1().child(t.name.clone()))
                .child(icon_button(format!("tag-edit-{}", t.id), IconName::Settings, "編集", true).on_click(cx.listener(move |this, _, w, cx| {
                    this.open_editor(Draft { id: Some(edit_t.id.clone()), name: edit_t.name.clone(), color: edit_t.color.clone(), ..Draft::new(EditKind::Tag) }, w, cx)
                })))
                .child(icon_button(format!("tag-delete-{}", t.id), IconName::Delete, "削除", true).on_click(cx.listener(move |this, _, _, cx| {
                    this.deleting_tag = Some(del_t.clone());
                    cx.notify();
                })))
        }));
        root = root
            .child(section_header(
                "タグ",
                form::button("tag-add", "＋ 追加", true, true).on_click(cx.listener(|this, _, w, cx| {
                    this.open_editor(Draft { color: random_color(), ..Draft::new(EditKind::Tag) }, w, cx)
                })),
            ))
            .child(if self.tags.is_empty() { div().mb_8().text_sm().text_color(n400()).child("タグがありません") } else { tags });

        if let Some(t) = self.deleting_tag.clone() {
            root = root.child(
                div()
                    .mb_8()
                    .p_4()
                    .rounded_lg()
                    .border_1()
                    .border_color(gpui::rgb(0xfecaca))
                    .bg(red50())
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("タグを削除しますか？"))
                    .child(div().text_xs().text_color(n700()).child(format!("「{}」を削除します。記録した時間は残りますが、エントリーとプロジェクトからこのタグが外れます。", t.name)))
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(form::button("tag-delete-cancel", "キャンセル", false, true).on_click(cx.listener(|this, _, _, cx| {
                                this.deleting_tag = None;
                                cx.notify();
                            })))
                            .child(form::button("tag-delete-confirm", "タグを削除", true, true).on_click(cx.listener(move |this, _, _, cx| {
                                this.deleting_tag = None;
                                let api = api(&this.state, cx);
                                this.write(api.delete_tag(t.id.clone()), false, cx)
                            }))),
                    ),
            );
        }
        root
    }
}
