//! 「プロジェクト」: clients with their projects in one list (left) and the selected project's form
//! (right). Changes save at once: colour, client and archive on click, the name on Enter or when
//! the field loses focus. A client is renamed in the list (double-click) and archived with the icon
//! beside its ＋. Clients and projects are archived, never deleted (like the Tauri screen).
use crate::app::AppState;
use crate::data::{DataClient, DataResult};
use crate::ui::theme::*;
use gpui::{
    AnyElement, AppContext, Context, Div, Entity, Focusable, FontWeight, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::color_picker::{ColorPickerEvent, ColorPickerState};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::searchable_list::SearchableListItem;
use gpui_component::select::{Select, SelectEvent, SelectState};
use gpui_component::{Icon, IconName, IndexPath, Sizable};
use std::future::Future;
use track_core::model::{Client, ClientPatch, Project, ProjectPatch};

/// The project colours (shared with track-cli).
pub use track_core::edit::PROJECT_COLORS as COLORS;

/// A client in the project's client select.
#[derive(Clone)]
pub struct ClientItem {
    id: String,
    name: SharedString,
}

impl SearchableListItem for ClientItem {
    type Value = String;

    fn title(&self) -> SharedString {
        self.name.clone()
    }

    fn value(&self) -> &String {
        &self.id
    }
}

type ClientSelect = SelectState<Vec<ClientItem>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    Project(String),
}

pub struct ProjectSettings {
    state: Entity<AppState>,
    pub clients: Vec<Client>,
    pub projects: Vec<Project>,
    pub selected: Option<Selection>,
    /// The name field of the selected item, and which item it was made for.
    pub name: Option<Entity<InputState>>,
    name_for: Option<Selection>,
    /// The saved name the field last showed; a field still holding it has not been typed in.
    name_synced: String,
    /// Focus the name field once it exists (just added).
    focus_name: bool,
    /// The eyedropper's state for the selected project.
    pub picker: Option<Entity<ColorPickerState>>,
    picker_for: Option<String>,
    /// The client select for the selected project, and the project and choices it was made for.
    pub client_select: Option<Entity<ClientSelect>>,
    client_select_for: Option<(String, Vec<(String, String)>)>,
    /// The colour being picked; saved when the popover closes, not on every slider step.
    pending_color: Option<(String, gpui::Hsla)>,
    pub error: Option<String>,
    /// A save failed: put the saved name back even while the field is focused.
    revert_name: bool,
    show_archived: bool,
    loaded: bool,
    /// Finished re-reads (tests check that a save reads once).
    pub reloads: usize,
    /// The client being renamed in the list, with its field.
    pub client_edit: Option<(String, Entity<InputState>)>,
    /// A client just added: rename it once it has been read.
    rename_next: Option<String>,
}

fn api(state: &Entity<AppState>, cx: &gpui::App) -> DataClient {
    state.read(cx).api.clone()
}

impl ProjectSettings {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        // Restores and outside writes (CLI, AI) re-read everything; follow them.
        cx.subscribe(&state, |this, _, _: &crate::app::StateEvent, cx| this.reload(cx)).detach();
        let mut s = ProjectSettings {
            state,
            clients: vec![],
            projects: vec![],
            selected: None,
            name: None,
            name_for: None,
            name_synced: String::new(),
            focus_name: false,
            picker: None,
            picker_for: None,
            client_select: None,
            client_select_for: None,
            pending_color: None,
            error: None,
            revert_name: false,
            show_archived: false,
            loaded: false,
            reloads: 0,
            client_edit: None,
            rename_next: None,
        };
        s.reload(cx);
        s
    }

    /// Re-reads clients and projects (archived included), keeping the selection by id.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let api = api(&self.state, cx);
        let (c, p) = (api.clients(), api.projects_all());
        cx.spawn(async move |this, cx| {
            let result = async { Ok::<_, crate::data::DataError>((c.await?, p.await?)) }.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok((c, p)) => {
                        (this.clients, this.projects) = (c, p);
                        if !this.loaded {
                            this.loaded = true;
                            if this.selected.is_none() {
                                this.selected = this.projects.iter().find(|p| !p.archived).map(|p| Selection::Project(p.id.clone()));
                            }
                        }
                        // A selection that no longer exists is dropped; an archived one opens the archive.
                        match this.selected.clone() {
                            Some(Selection::Project(id)) => match this.projects.iter().find(|p| p.id == id) {
                                None => this.selected = None,
                                Some(p) if p.archived => this.show_archived = true,
                                _ => {}
                            },
                            None => {}
                        }
                    }
                    Err(e) => this.error = Some(e.message),
                }
                this.reloads += 1;
                cx.notify();
            });
        })
        .detach();
    }

    /// Runs a write. Success clears the error and refreshes the main window; failure shows the
    /// message. Either way the lists are re-read, so the details show what is saved.
    fn write<T: Send + 'static>(&mut self, fut: impl Future<Output = DataResult<T>> + Send + 'static, cx: &mut Context<Self>) {
        self.write_with(fut, false, cx)
    }

    /// `name_save`: a failed save puts the saved name back in the field (other failures leave
    /// what is being typed alone).
    fn write_with<T: Send + 'static>(&mut self, fut: impl Future<Output = DataResult<T>> + Send + 'static, name_save: bool, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let result = fut.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    // The refresh re-reads this page too (StateEvent).
                    Ok(_) => {
                        this.error = None;
                        this.state.update(cx, |s, cx| s.refresh_all(cx));
                    }
                    Err(e) => {
                        this.revert_name = name_save;
                        this.reload(cx);
                        this.error = Some(match e.code.as_str() {
                            "invalid_input" => "名前は100文字以内で入力してください".to_string(),
                            _ => e.message,
                        })
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn select(&mut self, s: Selection, window: &mut Window, cx: &mut Context<Self>) {
        // Clicking another row does not blur the field, so save what was typed first.
        self.commit_name(window, cx);
        self.cancel_client_edit(cx);
        self.save_pending_color(cx);
        self.selected = Some(s);
        self.error = None;
        cx.notify();
    }

    fn project(&self, id: &str) -> Option<&Project> {
        self.projects.iter().find(|p| p.id == id)
    }

    fn client(&self, id: &str) -> Option<&Client> {
        self.clients.iter().find(|c| c.id == id)
    }

    /// The saved name of the selected item.
    fn saved_name(&self) -> Option<String> {
        match self.selected.as_ref()? {
            Selection::Project(id) => self.project(id).map(|p| p.name.clone()),
        }
    }

    fn add_project(&mut self, client_id: String, cx: &mut Context<Self>) {
        let color = track_core::edit::next_project_color(self.projects.len()).to_string();
        let fut = api(&self.state, cx).create_project(client_id, "新しいプロジェクト".into(), color, vec![]);
        self.after_create(fut, |this, id| {
            this.selected = Some(Selection::Project(id));
            this.focus_name = true;
        }, cx);
    }

    /// Adds a client and starts renaming it in the list.
    fn add_client(&mut self, cx: &mut Context<Self>) {
        let fut = api(&self.state, cx).create_client("新しいクライアント".into());
        self.after_create(fut, |this, id| this.rename_next = Some(id), cx);
    }

    /// Creates, then hands the new id to `then` (select it, rename it...).
    fn after_create(&mut self, fut: impl Future<Output = DataResult<String>> + Send + 'static, then: fn(&mut Self, String), cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let result = fut.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(id) => {
                        then(this, id);
                        this.error = None;
                        this.state.update(cx, |s, cx| s.refresh_all(cx));
                    }
                    // On success the refresh re-reads this page (StateEvent).
                    Err(e) => {
                        this.error = Some(e.message);
                        this.reload(cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Saves the name field for the selected item; an empty name puts the saved one back.
    fn commit_name(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(input), Some(sel), Some(saved)) = (self.name.clone(), self.selected.clone(), self.saved_name()) else { return };
        if self.name_for.as_ref() != Some(&sel) {
            return;
        }
        let raw = input.read(cx).value().to_string();
        if raw == self.name_synced {
            return;
        }
        let value = raw.trim().to_string();
        if value.is_empty() {
            input.update(cx, |i, cx| i.set_value(saved, window, cx));
            return;
        }
        if value == saved {
            return;
        }
        let api = api(&self.state, cx);
        match sel {
            Selection::Project(id) => self.write_with(api.update_project(id, ProjectPatch { name: Some(value), ..Default::default() }), true, cx),
        }
    }

    fn save_pending_color(&mut self, cx: &mut Context<Self>) {
        if let Some((id, c)) = self.pending_color.take() {
            let api = api(&self.state, cx);
            self.write(api.update_project(id, ProjectPatch { color: Some(to_hex(c)), ..Default::default() }), cx);
        }
    }

    /// Keeps the name field and the eyedropper in step with the selection and the saved values.
    fn sync_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(sel) = self.selected.clone() else {
            (self.name, self.name_for, self.picker, self.picker_for) = (None, None, None, None);
            return;
        };
        // Just added and not read yet: no field until there is a saved name to show.
        let Some(saved) = self.saved_name() else {
            (self.name, self.name_for) = (None, None);
            return;
        };
        if self.name_for.as_ref() != Some(&sel) || self.name.is_none() {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("名前").default_value(saved.clone()));
            cx.subscribe_in(&input, window, |this, _, ev: &InputEvent, window, cx| {
                if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                    this.commit_name(window, cx);
                }
            })
            .detach();
            (self.name, self.name_for, self.name_synced) = (Some(input), Some(sel.clone()), saved);
        } else if let Some(input) = self.name.clone() {
            // Show what is saved (a failed save, an outside change...) unless the user is typing.
            let value = input.read(cx).value().to_string();
            let focused = input.read(cx).focus_handle(cx).is_focused(window);
            let typed = value != self.name_synced;
            if value == saved {
                self.name_synced = saved;
            } else if !focused || !typed || self.revert_name {
                input.update(cx, |i, cx| i.set_value(saved.clone(), window, cx));
                self.name_synced = saved;
            }
        }
        self.revert_name = false;
        if self.focus_name {
            self.focus_name = false;
            if let Some(input) = self.name.clone() {
                input.update(cx, |i, cx| i.focus(window, cx));
            }
        }
        match &sel {
            Selection::Project(id) => {
                if self.picker_for.as_deref() != Some(id.as_str()) {
                    let initial = self.project(id).map(|p| hex(&p.color)).unwrap_or_else(|| hex(COLORS[0]));
                    let picker = cx.new(|cx| ColorPickerState::new(window, cx).default_value(initial));
                    let pid = id.clone();
                    cx.subscribe(&picker, move |this, _, ev: &ColorPickerEvent, _| {
                        if let ColorPickerEvent::Change(Some(c)) = ev {
                            this.pending_color = Some((pid.clone(), *c));
                        }
                    })
                    .detach();
                    cx.observe(&picker, |this, picker, cx| {
                        if !picker.read(cx).is_open() {
                            this.save_pending_color(cx);
                        }
                    })
                    .detach();
                    (self.picker, self.picker_for) = (Some(picker), Some(id.clone()));
                } else if let (Some(picker), Some(pj)) = (self.picker.clone(), self.project(id)) {
                    // Follow a preset click or an outside change, but not while picking.
                    let (open, shown) = picker.read_with(cx, |s, _| (s.is_open(), s.value().map(to_hex)));
                    let color = pj.color.to_lowercase();
                    if !open && self.pending_color.is_none() && shown.as_deref() != Some(color.as_str()) {
                        picker.update(cx, |s, cx| s.set_value(hex(&color), window, cx));
                    }
                }
            }
        }
        self.sync_client_select(&sel, window, cx);
    }

    /// The selected project's client select: archived clients are offered only when the project
    /// already belongs to one. Picking another client saves at once.
    fn sync_client_select(&mut self, sel: &Selection, window: &mut Window, cx: &mut Context<Self>) {
        let Selection::Project(id) = sel;
        let Some(current) = self.project(id).map(|p| p.client_id.clone()) else { return };
        let choices: Vec<(String, String)> = self.clients.iter().filter(|c| !c.archived || c.id == current).map(|c| (c.id.clone(), c.name.clone())).collect();
        let key = (id.clone(), choices.clone());
        if self.client_select_for.as_ref() != Some(&key) {
            let at = choices.iter().position(|(cid, _)| *cid == current).map(IndexPath::new);
            let items: Vec<ClientItem> = choices.into_iter().map(|(id, name)| ClientItem { id, name: name.into() }).collect();
            let select = cx.new(|cx| SelectState::new(items, at, window, cx));
            let pid = id.clone();
            cx.subscribe(&select, move |this, _, ev: &SelectEvent<Vec<ClientItem>>, cx| {
                let SelectEvent::Confirm(Some(cid)) = ev else { return };
                if this.project(&pid).is_some_and(|p| p.client_id != *cid) {
                    let api = api(&this.state, cx);
                    this.write(api.update_project(pid.clone(), ProjectPatch { client_id: Some(cid.clone()), ..Default::default() }), cx);
                }
            })
            .detach();
            (self.client_select, self.client_select_for) = (Some(select), Some(key));
        } else if let Some(select) = self.client_select.clone()
            && select.read(cx).selected_value() != Some(&current)
        {
            // A failed save or an outside change: show the saved client.
            select.update(cx, |s, cx| s.set_selected_value(&current, window, cx));
        }
    }

    /// Opens the client's name field in the list (saving any other one first).
    fn start_client_edit(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.client_edit.as_ref().is_some_and(|(e, _)| *e == id) {
            return;
        }
        self.cancel_client_edit(cx);
        let Some(name) = self.client(&id).map(|c| c.name.clone()) else { return };
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("クライアント名").default_value(name));
        cx.subscribe_in(&input, window, |this, _, ev: &InputEvent, window, cx| {
            // Only Enter saves; leaving the field cancels (so do Escape and clicking elsewhere).
            match ev {
                InputEvent::PressEnter { .. } => this.commit_client_edit(window, cx),
                InputEvent::Blur => this.cancel_client_edit(cx),
                _ => {}
            }
        })
        .detach();
        input.update(cx, |i, cx| i.focus(window, cx));
        self.client_edit = Some((id, input));
        cx.notify();
    }

    /// Saves the client name being edited and closes the field; an empty or unchanged name saves
    /// nothing.
    fn commit_client_edit(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some((id, input)) = self.client_edit.take() else { return };
        let value = input.read(cx).value().trim().to_string();
        if !value.is_empty() && self.client(&id).is_some_and(|c| c.name != value) {
            let api = api(&self.state, cx);
            self.write(api.update_client(id, ClientPatch { name: Some(value), ..Default::default() }), cx);
        }
        cx.notify();
    }

    /// Closes the client name field without saving.
    fn cancel_client_edit(&mut self, cx: &mut Context<Self>) {
        if self.client_edit.take().is_some() {
            cx.notify();
        }
    }

    fn toggle_client_archive(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(archived) = self.client(id).map(|c| c.archived) else { return };
        let api = api(&self.state, cx);
        self.write(api.update_client(id.to_string(), ClientPatch { archived: Some(!archived), ..Default::default() }), cx);
    }

    fn render_tree(&self, p: Palette, cx: &mut Context<Self>) -> Div {
        let row = |id: String, selected: bool| {
            let sel_id = id.clone();
            div()
                .id(SharedString::from(id))
                .debug_selector(move || sel_id.clone())
                .flex()
                .items_center()
                .gap_2()
                .h(px(28.))
                .px_3()
                .rounded(px(5.))
                .cursor_pointer()
                .map(|d| if selected { d.bg(p.selected) } else { d.hover(move |s| s.bg(p.hover)) })
        };
        let project_row = |pj: &Project, cx: &mut Context<Self>| {
            let id = pj.id.clone();
            let on = self.selected == Some(Selection::Project(id.clone()));
            row(format!("tree-project-{}", pj.id), on)
                .pl(px(22.))
                .child(div().size(px(9.)).flex_none().rounded_full().bg(hex(&pj.color)))
                .child(div().min_w_0().truncate().text_sm().child(pj.name.clone()))
                .on_click(cx.listener(move |this, _, window, cx| this.select(Selection::Project(id.clone()), window, cx)))
        };
        let icon_button = |id: String, icon: Icon, tip: &'static str| {
            let sel = id.clone();
            div()
                .id(SharedString::from(id))
                .debug_selector(move || sel.clone())
                .px_1()
                .rounded(px(4.))
                .hover(move |s| s.bg(p.hover))
                .child(icon.size_3p5())
                .tooltip(move |window, cx| gpui_component::tooltip::Tooltip::new(tip).build(window, cx))
        };
        let client_block = |c: &Client, projects: Vec<&Project>, cx: &mut Context<Self>| {
            let (id, add_id, arch_id) = (c.id.clone(), c.id.clone(), c.id.clone());
            let archived = c.archived;
            let editing = self.client_edit.as_ref().filter(|(e, _)| *e == c.id).map(|(_, i)| i.clone());
            let name = match editing {
                Some(input) => div()
                    .flex_1()
                    .min_w_0()
                    .debug_selector(|| "tree-client-name".into())
                    // The input lets Escape through when it has nothing of its own to close.
                    .on_action(cx.listener(|this, _: &gpui_component::input::Escape, _, cx| this.cancel_client_edit(cx)))
                    .on_mouse_down_out(cx.listener(|this, _, _, cx| this.cancel_client_edit(cx)))
                    .child(Input::new(&input).small())
                    .into_any_element(),
                None => div().flex_1().min_w_0().truncate().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(p.text_muted).child(c.name.clone()).into_any_element(),
            };
            let archive_icon = if archived { crate::ui::icons::archive_restore() } else { crate::ui::icons::archive() };
            let archive_tip = if archived { "アーカイブを戻す" } else { "アーカイブする（配下のプロジェクトも）" };
            div()
                .flex()
                .flex_col()
                .child(
                    row(format!("tree-client-{}", c.id), false)
                        .mt_2()
                        .cursor_default()
                        .child(name)
                        .child(
                            icon_button(format!("tree-client-archive-{}", c.id), archive_icon, archive_tip)
                                .text_color(p.text_muted)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.toggle_client_archive(&arch_id, cx)
                                })),
                        )
                        // An archived client takes no new projects.
                        .when(!archived, |d| {
                            d.child(
                                icon_button(format!("tree-client-add-{}", c.id), Icon::new(IconName::Plus), "プロジェクトを追加")
                                    .text_color(p.accent)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.add_project(add_id.clone(), cx)
                                    })),
                            )
                        })
                        // Double-click renames the client in place.
                        .on_click(cx.listener(move |this, ev: &gpui::ClickEvent, window, cx| {
                            if ev.click_count() >= 2 {
                                this.start_client_edit(id.clone(), window, cx);
                            }
                        })),
                )
                .children(projects.into_iter().map(|pj| project_row(pj, cx)))
        };
        let active: Vec<&Client> = self.clients.iter().filter(|c| !c.archived).collect();
        let archived_clients: Vec<&Client> = self.clients.iter().filter(|c| c.archived).collect();
        let archived_count = archived_clients.len() + self.projects.iter().filter(|p| p.archived).count();
        let mut col = div().flex().flex_col().gap(px(1.)).children(active.iter().map(|c| {
            let ps = self.projects.iter().filter(|p| p.client_id == c.id && !p.archived).collect();
            client_block(c, ps, cx)
        }));
        col = col.child(
            row("tree-add-client".into(), false)
                .mt_3()
                .text_sm()
                .text_color(p.text_muted)
                .child(Icon::new(IconName::Plus).size_3p5())
                .child("クライアント")
                .on_click(cx.listener(|this, _, _, cx| this.add_client(cx))),
        );
        if archived_count > 0 {
            col = col.child(
                row("tree-archived".into(), false)
                    .mt_2()
                    .text_sm()
                    .text_color(p.text_muted)
                    .child(Icon::new(if self.show_archived { IconName::ChevronDown } else { IconName::ChevronRight }).size_3p5())
                    .child(format!("アーカイブ（{archived_count}）"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_archived = !this.show_archived;
                        cx.notify();
                    })),
            );
            if self.show_archived {
                // Archived clients with their projects, then archived projects of active clients.
                col = col.children(archived_clients.iter().map(|c| {
                    let ps = self.projects.iter().filter(|p| p.client_id == c.id).collect();
                    client_block(c, ps, cx).opacity(0.7)
                }));
                col = col.children(active.iter().filter_map(|c| {
                    let ps: Vec<&Project> = self.projects.iter().filter(|p| p.client_id == c.id && p.archived).collect();
                    (!ps.is_empty()).then(|| {
                        div()
                            .opacity(0.7)
                            .child(div().mt_2().px_3().text_xs().text_color(p.text_faint).child(c.name.clone()))
                            .children(ps.into_iter().map(|pj| project_row(pj, cx)))
                    })
                }));
            }
        }
        col
    }

    fn render_detail(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let label = |t: &'static str| div().text_sm().font_weight(FontWeight::MEDIUM).text_color(p.text_muted).child(t);
        let line = |l: &'static str, control: AnyElement| div().flex().flex_col().gap_1p5().py_2().child(label(l)).child(control);
        let name_field = self.name.as_ref().map(|n| div().debug_selector(|| "detail-name".into()).child(Input::new(n)).into_any_element()).unwrap_or_else(|| div().into_any_element());
        let error = self.error.clone().map(|e| div().debug_selector(|| "detail-error".into()).mt_2().text_sm().text_color(p.danger).child(e));
        let archive_button = |archived: bool, enabled: bool, note: Option<&'static str>, cx: &mut Context<Self>| {
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    super::form::button_with_icon(
                        p,
                        "detail-archive",
                        if archived { crate::ui::icons::archive_restore() } else { crate::ui::icons::archive() },
                        if archived { "アーカイブを戻す" } else { "アーカイブする" },
                        false,
                        enabled,
                    )
                        .debug_selector(|| "detail-archive".into())
                        .when(enabled, |d| d.on_click(cx.listener(|this, _, _, cx| this.toggle_archive(cx)))),
                )
                .children(note.map(|n| div().text_xs().text_color(p.text_faint).child(n)))
        };
        match self.selected.clone() {
            Some(Selection::Project(id)) => {
                let Some(pj) = self.project(&id).cloned() else { return div().into_any_element() };
                let custom = !COLORS.iter().any(|c| c.eq_ignore_ascii_case(&pj.color));
                let swatches = div().flex().flex_wrap().items_center().gap_2().children(COLORS.iter().enumerate().map(|(i, c)| {
                    let (sid, color, pid) = (format!("detail-color-{i}"), c.to_string(), id.clone());
                    let sid2 = sid.clone();
                    div()
                        .id(SharedString::from(sid))
                        .debug_selector(move || sid2.clone())
                        .size(px(24.))
                        .rounded_full()
                        .cursor_pointer()
                        .bg(hex(c))
                        .when(pj.color.eq_ignore_ascii_case(c), |d| d.border_2().border_color(p.text))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let api = api(&this.state, cx);
                            this.write(api.update_project(pid.clone(), ProjectPatch { color: Some(color.clone()), ..Default::default() }), cx)
                        }))
                }));
                let swatches = match self.picker.clone() {
                    Some(picker) => swatches.child(super::color_field::eyedropper("detail-color-popover", &picker, custom.then(|| gpui::Hsla::from(hex(&pj.color))), p, cx)),
                    None => swatches,
                };
                let client_archived = self.client(&pj.client_id).is_some_and(|c| c.archived);
                let client = match self.client_select.as_ref() {
                    Some(select) => div().debug_selector(|| "detail-client".into()).w(px(240.)).child(Select::new(select)).into_any_element(),
                    None => div().into_any_element(),
                };
                div()
                    .flex()
                    .flex_col()
                    .child(div().debug_selector(|| "detail-heading".into()).text_lg().font_weight(FontWeight::SEMIBOLD).mb_2().child("プロジェクトを編集"))
                    .child(line("名前", name_field))
                    .child(line("クライアント", client))
                    .child(line("色", swatches.into_any_element()))
                    .child(div().mt_4().child(archive_button(pj.archived, !(pj.archived && client_archived), (pj.archived && client_archived).then_some("クライアントのアーカイブを先に戻してください"), cx)))
                    .children(error)
                    .into_any_element()
            }
            None => div()
                .debug_selector(|| "detail-empty".into())
                .text_sm()
                .text_color(p.text_muted)
                .child("左からプロジェクトを選ぶか、クライアントの ＋ で追加してください。")
                .into_any_element(),
        }
    }

    fn toggle_archive(&mut self, cx: &mut Context<Self>) {
        let api = api(&self.state, cx);
        match self.selected.clone() {
            Some(Selection::Project(id)) => {
                let Some(archived) = self.project(&id).map(|p| p.archived) else { return };
                self.write(api.update_project(id, ProjectPatch { archived: Some(!archived), ..Default::default() }), cx)
            }
            None => {}
        }
    }
}

impl Render for ProjectSettings {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        self.sync_fields(window, cx);
        if let Some(id) = self.rename_next.clone()
            && self.client(&id).is_some()
        {
            self.rename_next = None;
            self.start_client_edit(id, window, cx);
        }
        div()
            .flex()
            .items_start()
            .gap_6()
            .child(div().w(px(240.)).flex_none().py_1().border_r_1().border_color(p.border_subtle).pr_3().child(self.render_tree(p, cx)))
            .child(div().flex_1().min_w_0().child(self.render_detail(p, cx)))
    }
}
