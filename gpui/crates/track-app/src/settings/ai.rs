//! 「AI」: the generation provider (SettingsPage「生成プロバイダー」). Saved to config.json.
use super::form::{self, Actions};
use crate::app::AppState;
use crate::ui::theme::*;
use gpui::{
    AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Task,
    Window, div, prelude::FluentBuilder, px,
};
use gpui_component::IndexPath;
use gpui_component::input::{Input, InputState, Textarea, TextareaState};
use gpui_component::searchable_list::SearchableListItem;
use gpui_component::select::{Select, SelectEvent, SelectState};
use std::time::Duration;
use track_core::ai::models::{CodexModelCatalog, codex_models, default_codex_home};
use track_core::ai::{AiStatus, provider_from_config};
use track_core::config::{Config, ConfigPatch};

const PROVIDERS: [(&str, &str); 3] = [("apple-intelligence", "Apple Intelligence"), ("codex", "Codex"), ("custom-command", "カスタムコマンド")];
/// models.ts: the catalog refreshes every 5 minutes while shown.
const MODELS_REFRESH: Duration = Duration::from_secs(5 * 60);

/// A model in the Codex model select (`""` = Codex's default model).
#[derive(Clone)]
pub struct ModelItem {
    id: String,
    label: SharedString,
}

impl SearchableListItem for ModelItem {
    type Value = String;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn value(&self) -> &String {
        &self.id
    }
}

pub struct AiSettings {
    state: Entity<AppState>,
    pub provider: String,
    /// The chosen Codex model (`""` = Codex's default).
    pub codex_model: String,
    /// The model select and the choices it was built with.
    pub model_select: Option<Entity<SelectState<Vec<ModelItem>>>>,
    model_choices: Vec<(String, String)>,
    pub codex_executable: Entity<InputState>,
    pub command_executable: Entity<InputState>,
    pub command_args: Entity<TextareaState>,
    saved: Config,
    pub status: Option<AiStatus>,
    pub models: Option<CodexModelCatalog>,
    models_loading: bool,
    saving: bool,
    save_error: Option<String>,
    _models_timer: Option<Task<()>>,
}

fn args_from_text(text: &str) -> Vec<String> {
    text.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect()
}

impl AiSettings {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let saved = state.read(cx).api.full_config();
        let input = |v: &str, ph: &str, window: &mut Window, cx: &mut Context<Self>| {
            let e = cx.new(|cx| InputState::new(window, cx).placeholder(ph.to_string()).default_value(v.to_string()));
            cx.observe(&e, |_, _, cx| cx.notify()).detach();
            e
        };
        let codex_model = saved.ai_codex_model.clone();
        let codex_executable = input(&saved.ai_codex_executable, "自動検出", window, cx);
        let command_executable = input(&saved.ai_command_executable, "/path/to/command", window, cx);
        let command_args = cx.new(|cx| TextareaState::new(window, cx).rows(4).default_value(saved.ai_command_args.join("\n")));
        cx.observe(&command_args, |_, _, cx| cx.notify()).detach();
        let mut s = AiSettings {
            state,
            provider: saved.ai_provider.clone(),
            codex_model,
            model_select: None,
            model_choices: vec![],
            codex_executable,
            command_executable,
            command_args,
            saved,
            status: None,
            models: None,
            models_loading: false,
            saving: false,
            save_error: None,
            _models_timer: None,
        };
        s.refresh_status(cx);
        if s.provider == "codex" {
            s.start_models(cx);
        }
        s
    }

    fn draft(&self, cx: &gpui::App) -> ConfigPatch {
        ConfigPatch {
            ai_provider: Some(self.provider.clone()),
            ai_codex_model: Some(self.codex_model.trim().to_string()),
            ai_codex_executable: Some(self.codex_executable.read(cx).value().trim().to_string()),
            ai_command_executable: Some(self.command_executable.read(cx).value().trim().to_string()),
            ai_command_args: Some(args_from_text(&self.command_args.read(cx).value())),
            ..Default::default()
        }
    }

    fn dirty(&self, cx: &gpui::App) -> bool {
        let d = self.draft(cx);
        let s = &self.saved;
        d.ai_provider.as_deref() != Some(s.ai_provider.as_str())
            || d.ai_codex_model.as_deref() != Some(s.ai_codex_model.as_str())
            || d.ai_codex_executable.as_deref() != Some(s.ai_codex_executable.as_str())
            || d.ai_command_executable.as_deref() != Some(s.ai_command_executable.as_str())
            || d.ai_command_args.as_ref() != Some(&s.ai_command_args)
    }

    /// The Tauri screen's client-side checks.
    fn validation(&self, cx: &gpui::App) -> Option<&'static str> {
        let d = self.draft(cx);
        if self.provider == "custom-command" && d.ai_command_executable.as_deref().unwrap_or("").is_empty() {
            Some("実行するコマンドを指定してください")
        } else if d.ai_command_args.as_ref().is_some_and(|a| a.len() > 32) {
            Some("引数は32個以内で指定してください")
        } else {
            None
        }
    }

    /// Provider status for the saved configuration (resolving executables may run `--version`).
    fn refresh_status(&mut self, cx: &mut Context<Self>) {
        let app_config = self.saved.app_config();
        let task = cx.background_spawn(async move { provider_from_config(&app_config).status() });
        cx.spawn(async move |this, cx| {
            let status = task.await;
            let _ = this.update(cx, |this, cx| {
                this.status = Some(status);
                cx.notify();
            });
        })
        .detach();
    }

    fn load_models(&mut self, cx: &mut Context<Self>) {
        if self.models_loading {
            return;
        }
        self.models_loading = true;
        let exe = self.saved.ai_codex_executable.clone();
        let home = dirs::home_dir().unwrap_or_default();
        let task = cx.background_spawn(async move { codex_models(&exe, &home, &default_codex_home(&home)) });
        cx.spawn(async move |this, cx| {
            let catalog = task.await;
            let _ = this.update(cx, |this, cx| {
                this.models_loading = false;
                this.models = Some(catalog);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn start_models(&mut self, cx: &mut Context<Self>) {
        self.load_models(cx);
        if self._models_timer.is_none() {
            self._models_timer = Some(cx.spawn(async move |this, cx| loop {
                cx.background_executor().timer(MODELS_REFRESH).await;
                if this.update(cx, |this, cx| this.load_models(cx)).is_err() {
                    break;
                }
            }));
        }
    }

    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let s = self.saved.clone();
        self.provider = s.ai_provider.clone();
        self.codex_model = s.ai_codex_model.clone();
        self.codex_executable.update(cx, |i, cx| i.set_value(s.ai_codex_executable.clone(), window, cx));
        self.command_executable.update(cx, |i, cx| i.set_value(s.ai_command_executable.clone(), window, cx));
        self.command_args.update(cx, |i, cx| i.set_value(s.ai_command_args.join("\n"), window, cx));
        self.save_error = None;
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        if self.validation(cx).is_some() || self.saving {
            return;
        }
        self.saving = true;
        let fut = self.state.read(cx).api.save_config(self.draft(cx));
        cx.spawn(async move |this, cx| {
            let result = fut.await;
            let _ = this.update(cx, |this, cx| {
                this.saving = false;
                match result {
                    Ok(c) => {
                        let codex_changed = c.ai_codex_executable != this.saved.ai_codex_executable;
                        this.saved = c;
                        this.save_error = None;
                        this.refresh_status(cx);
                        if this.provider == "codex" && (codex_changed || this.models.is_none()) {
                            this.start_models(cx);
                        }
                        this.state.update(cx, |s, cx| s.refresh_all(cx));
                    }
                    Err(e) => this.save_error = Some(e.message),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// The model select: Codex's default, the catalog's models, and the chosen model when the
    /// catalog does not list it. Rebuilt when those change; picking one is a change to save.
    fn sync_model_select(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut choices = vec![(String::new(), "Codexの既定モデル".to_string())];
        choices.extend(self.models.iter().flat_map(|c| c.models.iter().cloned()));
        if !self.codex_model.is_empty() && !choices.iter().any(|(id, _)| *id == self.codex_model) {
            choices.push((self.codex_model.clone(), self.codex_model.clone()));
        }
        if self.model_select.is_none() || choices != self.model_choices {
            let at = choices.iter().position(|(id, _)| *id == self.codex_model).map(IndexPath::new);
            let items: Vec<ModelItem> = choices.iter().map(|(id, label)| ModelItem { id: id.clone(), label: label.clone().into() }).collect();
            let select = cx.new(|cx| SelectState::new(items, at, window, cx));
            cx.subscribe(&select, |this, _, ev: &SelectEvent<Vec<ModelItem>>, cx| {
                if let SelectEvent::Confirm(Some(id)) = ev {
                    this.codex_model = id.clone();
                    cx.notify();
                }
            })
            .detach();
            (self.model_select, self.model_choices) = (Some(select), choices);
        } else if let Some(select) = self.model_select.clone()
            && select.read(cx).selected_value() != Some(&self.codex_model)
        {
            // 変更を破棄 put the saved model back.
            let model = self.codex_model.clone();
            select.update(cx, |s, cx| s.set_selected_value(&model, window, cx));
        }
    }

    fn render_codex(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let detail = match (&self.models, self.models_loading) {
            (Some(c), _) => c.detail.clone(),
            (None, true) => "Codexのモデル一覧を取得しています…".into(),
            (None, false) => String::new(),
        };
        form::section(p, "ai-section-codex", "Codex", "利用するモデルと実行環境を設定します。", false)
            .child(form::field(p, 
                "ai-field-codex-model",
                "モデル",
                "Codexから取得した候補から選びます。既定ではCodex側の既定モデルを使います。",
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().debug_selector(|| "ai-codex-model".into()).w(px(320.)).children(self.model_select.as_ref().map(Select::new)))
                            .child(form::button(p, "codex-models-refresh", "更新", false, !self.models_loading).on_click(cx.listener(|this, _, _, cx| this.load_models(cx)))),
                    )
                    .child(div().text_xs().text_color(p.text_muted).child(detail))
                    .child(div().text_xs().text_color(p.text_muted).child("実行ファイルの変更を保存すると、そのCodexから候補を取得します。")),
            ))
            .child(form::field(p, 
                "ai-field-codex-executable",
                "codex実行ファイル",
                "空欄ではPATHとChatGPTアプリから新しいバージョンを自動検出します。",
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().w_full().child(Input::new(&self.codex_executable)))
                    .child(div().text_xs().text_color(p.text_muted).child("Codexの既存のログインを再利用し、TrackはAPIキーを保存しません。")),
            ))
    }
}

impl Render for AiSettings {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        if self.provider == "codex" {
            self.sync_model_select(window, cx);
        }
        let dirty = self.dirty(cx);
        let validation = self.validation(cx);
        let error = validation.map(str::to_string).or(self.save_error.clone());
        let can_save = dirty && validation.is_none() && !self.saving;
        let providers = div().flex().gap_2().children(PROVIDERS.iter().map(|(key, label)| {
            let on = self.provider == *key;
            let id = format!("ai-provider-{key}");
            let sel = id.clone();
            let key = key.to_string();
            div()
                .id(SharedString::from(id))
                .debug_selector(move || sel.clone())
                .px_3()
                .py_1()
                .rounded_md()
                .text_sm()
                .cursor_pointer()
                .when(on, |d| d.bg(p.primary).text_color(p.on_color))
                .when(!on, |d| d.border_1().border_color(p.border).text_color(p.text))
                .child(*label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.provider = key.clone();
                    if this.provider == "codex" && this.models.is_none() {
                        this.start_models(cx);
                    }
                    cx.notify();
                }))
        }));
        let mut root = div().child(
            form::section(p, "ai-section-provider", "生成プロバイダー", "メモタイトルと週報の生成に共通で使う実行環境です。", true)
                .child(form::field(p, "ai-field-provider", "プロバイダー", "利用するAI環境を選択します。", providers)),
        );
        root = match self.provider.as_str() {
            "codex" => root.child(self.render_codex(cx)),
            "custom-command" => root.child(
                form::section(p, "ai-section-command", "カスタムコマンド", "任意のローカルコマンドをAI処理に利用します。", false)
                    .child(form::field(p, "ai-field-command-executable", "実行ファイル", "AI処理に使用するコマンドの絶対パスです。", div().w_full().child(Input::new(&self.command_executable))))
                    .child(form::field(p, 
                        "ai-field-command-args",
                        "引数",
                        "コマンドへ渡す引数を1行に1つ入力します。",
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(div().h(px(100.)).child(Textarea::new(&self.command_args).h_full()))
                            .child(div().text_xs().text_color(p.text_muted).child(
                                "シェルは使わず、プロンプトを標準入力へ渡します。認証はコマンド側の既存設定を使います。利用するサービスの規約とライセンスは、ご自身で確認してください。",
                            )),
                    )),
            ),
            _ => root.child(
                form::section(p, "ai-section-apple", "Apple Intelligence", "Appleの端末内モデルを使うための設定です。", false)
                    .child(form::field(p, "ai-field-apple", "利用方法", "Appleの端末内モデルを利用します。", div().text_sm().text_color(p.text).child("macOS 26以降で利用できます。APIキーや外部通信は不要です。"))),
            ),
        };
        let status = (!dirty).then(|| self.status.clone()).flatten();
        root.when(validation.is_some() || status.is_some(), |d| {
            d.child(form::section(p, "ai-section-status", "状態", "現在の設定でAI機能を利用できるか確認します。", false).when_some(status, |d, s| {
                d.child(form::field(p, 
                    "ai-field-status",
                    "接続状態",
                    "",
                    div()
                        .debug_selector(|| "ai-status".into())
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .text_sm()
                        .when(s.available, |d| d.bg(p.success_bg).text_color(p.success))
                        .when(!s.available, |d| d.bg(p.warning_bg).text_color(p.warning_text))
                        .child(s.detail),
                ))
            }))
        })
        .child(form::actions(p, 
            Actions { prefix: "ai", dirty, saving: self.saving, error: error.as_deref() },
            form::button(p, "ai-discard", "変更を破棄", false, !self.saving).on_click(cx.listener(|this, _, w, cx| this.reset(w, cx))),
            form::button(p, "ai-save", if self.saving { "保存中…" } else { "変更を保存" }, true, can_save).on_click(cx.listener(|this, _, _, cx| this.save(cx))),
        ))
    }
}
