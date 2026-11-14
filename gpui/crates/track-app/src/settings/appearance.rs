//! 外観: 自動 / 明るい / 暗い (saved in gpui-config.json; 自動 follows macOS).
use crate::app::AppState;
use crate::settings::form;
use crate::ui::theme::*;
use gpui::{Context, Entity, InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder};
use track_core::config::{Appearance, ConfigPatch};

pub struct AppearanceSettings {
    state: Entity<AppState>,
    current: Appearance,
    error: Option<String>,
}

impl AppearanceSettings {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let current = state.read(cx).api.full_config().appearance;
        AppearanceSettings { state, current, error: None }
    }

    fn choose(&mut self, a: Appearance, cx: &mut Context<Self>) {
        let fut = self.state.read(cx).api.save_config(ConfigPatch { appearance: Some(a), ..Default::default() });
        cx.spawn(async move |this, cx| {
            let result = fut.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(saved) => {
                        this.current = saved.appearance;
                        this.error = None;
                        apply_setting(saved.appearance, is_dark(cx.window_appearance()), cx);
                    }
                    Err(e) => this.error = Some(format!("保存できませんでした: {}", e.message)),
                }
                cx.notify();
            });
        })
        .detach();
    }
}

impl Render for AppearanceSettings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let option = |a: Appearance, label: &str, cx: &mut Context<Self>| {
            let id = format!("appearance-{}", a.key());
            form::button(p, id.clone(), label, self.current == a, true)
                .debug_selector(move || id.clone())
                .on_click(cx.listener(move |this, _, _, cx| this.choose(a, cx)))
        };
        let buttons = div()
            .flex()
            .gap_2()
            .child(option(Appearance::System, "自動", cx))
            .child(option(Appearance::Light, "明るい", cx))
            .child(option(Appearance::Dark, "暗い", cx));
        div().flex().flex_col().gap_3().child(
            form::section(p, "appearance-section", "外観", "画面の明るさです。自動は macOS の設定に合わせます。", true)
                .child(form::field(p, "appearance-field-scheme", "配色", "明るい配色と暗い配色を切り替えます。", buttons)),
        )
        .when_some(appearance_override(), |d, v| {
            d.child(div().debug_selector(|| "appearance-override".into()).text_sm().text_color(p.text_muted).child(format!(
                "開発用の TRACK_GPUI_APPEARANCE={v} が設定されているため、画面は{}のままです（選択は保存されます）。",
                if v == "dark" { "暗い配色" } else { "明るい配色" }
            )))
        })
        .when_some(self.error.clone(), |d, e| d.child(div().debug_selector(|| "appearance-error".into()).text_sm().text_color(p.danger).child(e)))
    }
}
