//! 「AI」: the generation provider (SettingsPage「生成プロバイダー」). Saved to config.json.
use super::form::{self, Actions};
use crate::app::AppState;
use crate::ui::theme::*;
use gpui::{
    AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Task,
    Window, div, prelude::FluentBuilder, px,
};
use gpui_component::input::{Input, InputState, Textarea, TextareaState};
use std::time::Duration;
use track_core::ai::models::{CodexModelCatalog, codex_models, default_codex_home};
use track_core::ai::{AiStatus, provider_from_config};
use track_core::config::{Config, ConfigPatch};

const PROVIDERS: [(&str, &str); 3] = [("apple-intelligence", "Apple Intelligence"), ("codex", "Codex"), ("custom-command", "カスタムコマンド")];
/// models.ts: the catalog refreshes every 5 minutes while shown.
const MODELS_REFRESH: Duration = Duration::from_secs(5 * 60);

pub struct AiSettings {
    state: Entity<AppState>,
    pub provider: String,
    pub codex_model: Entity<InputState>,
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
        let codex_model = input(&saved.ai_codex_model, "Codexの既定モデル", window, cx);
        let codex_executable = input(&saved.ai_codex_executable, "自動検出", window, cx);
        let command_executable = input(&saved.ai_command_executable, "/path/to/command", window, cx);
        let command_args = cx.new(|cx| TextareaState::new(window, cx).rows(4).default_value(saved.ai_command_args.join("\n")));
        cx.observe(&command_args, |_, _, cx| cx.notify()).detach();
        let mut s = AiSettings {
            state,
            provider: saved.ai_provider.clone(),
            codex_model,
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
            ai_codex_model: Some(self.codex_model.read(cx).value().trim().to_string()),
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
        self.codex_model.update(cx, |i, cx| i.set_value(s.ai_codex_model.clone(), window, cx));
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

    fn render_codex(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let models = self.models.clone();
        let candidates = div().flex().flex_wrap().gap_2().children(models.iter().flat_map(|c| c.models.iter().cloned()).enumerate().map(|(i, (id, label))| {
            let sel = format!("codex-model-{i}");
            let sel2 = sel.clone();
            div()
                .id(SharedString::from(sel))
                .debug_selector(move || sel2.clone())
                .px_2()
                .py(px(2.))
                .rounded_md()
                .border_1()
                .border_color(n200())
                .text_xs()
                .cursor_pointer()
                .hover(|s| s.bg(n100()))
                .child(label)
                .on_click(cx.listener(move |this, _, w, cx| {
                    this.codex_model.update(cx, |m, cx| m.set_value(id.clone(), w, cx));
                    cx.notify();
                }))
        }));
        let detail = match (&self.models, self.models_loading) {
            (Some(c), _) => c.detail.clone(),
            (None, true) => "Codexのモデル一覧を取得しています…".into(),
            (None, false) => String::new(),
        };
        form::card("Codex", "利用するモデルと実行環境を設定します。")
            .child(form::row(
                "モデル",
                "空欄ではCodex側の既定モデルを使います。任意のモデルIDも入力できます。",
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().w(px(320.)).child(Input::new(&self.codex_model)))
                    .child(candidates)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().text_xs().text_color(n500()).child(detail))
                            .child(form::button("codex-models-refresh", "更新", false, !self.models_loading).on_click(cx.listener(|this, _, _, cx| this.load_models(cx)))),
                    )
                    .child(div().text_xs().text_color(n500()).child("実行ファイルの変更を保存すると、そのCodexから候補を取得します。")),
            ))
            .child(form::row(
                "codex実行ファイル",
                "空欄ではPATHとChatGPTアプリから新しいバージョンを自動検出します。",
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().w(px(420.)).child(Input::new(&self.codex_executable)))
                    .child(div().text_xs().text_color(n500()).child("Codexの既存のログインを再利用し、TrackはAPIキーを保存しません。")),
            ))
    }
}

impl Render for AiSettings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                .when(on, |d| d.bg(n900()).text_color(white()))
                .when(!on, |d| d.border_1().border_color(n200()).text_color(n700()))
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
            form::card("生成プロバイダー", "メモタイトルと週報の生成に共通で使う実行環境です。").child(form::row("プロバイダー", "利用するAI環境を選択します。", providers)),
        );
        root = match self.provider.as_str() {
            "codex" => root.child(self.render_codex(cx)),
            "custom-command" => root.child(
                form::card("カスタムコマンド", "任意のローカルコマンドをAI処理に利用します。")
                    .child(form::row("実行ファイル", "AI処理に使用するコマンドの絶対パスです。", div().w(px(420.)).child(Input::new(&self.command_executable))))
                    .child(form::row(
                        "引数",
                        "コマンドへ渡す引数を1行に1つ入力します。",
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(div().h(px(100.)).child(Textarea::new(&self.command_args).h_full()))
                            .child(div().text_xs().text_color(n500()).child(
                                "シェルは使わず、プロンプトを標準入力へ渡します。認証はコマンド側の既存設定を使います。利用するサービスの規約とライセンスは、ご自身で確認してください。",
                            )),
                    )),
            ),
            _ => root.child(
                form::card("Apple Intelligence", "Appleの端末内モデルを使うための設定です。")
                    .child(form::row("利用方法", "Appleの端末内モデルを利用します。", div().text_sm().text_color(n700()).child("macOS 26以降で利用できます。APIキーや外部通信は不要です。"))),
            ),
        };
        let status = (!dirty).then(|| self.status.clone()).flatten();
        root.when(validation.is_some() || status.is_some(), |d| {
            d.child(form::card("状態", "現在の設定でAI機能を利用できるか確認します。").when_some(status, |d, s| {
                d.child(form::row(
                    "接続状態",
                    "",
                    div()
                        .debug_selector(|| "ai-status".into())
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .text_sm()
                        .when(s.available, |d| d.bg(gpui::rgb(0xecfdf5)).text_color(gpui::rgb(0x065f46)))
                        .when(!s.available, |d| d.bg(gpui::rgb(0xfffbeb)).text_color(gpui::rgb(0x92400e)))
                        .child(s.detail),
                ))
            }))
        })
        .child(form::actions(
            Actions { prefix: "ai", dirty, saving: self.saving, error: error.as_deref() },
            form::button("ai-discard", "変更を破棄", false, !self.saving).on_click(cx.listener(|this, _, w, cx| this.reset(w, cx))),
            form::button("ai-save", if self.saving { "保存中…" } else { "変更を保存" }, true, can_save).on_click(cx.listener(|this, _, _, cx| this.save(cx))),
        ))
    }
}
