# GPUI 版 暗い配色と専用の設定ファイル Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** GPUI 版の設定（AI・バックアップ・外観）を専用の `gpui-config.json` に移し、暗い配色と「自動／明るい／暗い」の切り替えを足す。

**Architecture:** `track-core::config` の保存先を `gpui-config.json` に替え、無ければ `config.json` から一度だけ写す。`Config` に `appearance` を足す。アプリは `Palette::dark()` を持ち、外観設定と macOS の外観から明暗を決めて `Palette` グローバルと gpui-component の Theme を切り替える。

**Tech Stack:** Rust 1.98.1, GPUI (gpui-pre 0.3.12), gpui-component 0.9.1

**Spec:** `docs/superpowers/specs/2026-11-14-gpui-dark-and-own-config-design.md`

## Global Constraints

- GPUI 版は共有の `config.json` を書かない（読むのは初回の移行だけ）
- 移行で AI・バックアップの値を失わない。バックアップ先が開発用ガードの外なら既定（`<data_dir>/exports`）にする
- 外観の既定は「自動」。値は `"system" | "light" | "dark"`（JSON キー `appearance`）
- 開発用の上書き: 環境変数 `TRACK_GPUI_APPEARANCE=light|dark`
- 暗い配色も `light_palette_is_readable` と同じコントラスト基準（text/bg ≥ 7、text_muted/bg ≥ 4.5、text/surface ≥ 7、on_color/primary ≥ 7）
- 画面に生の色を書かない（`no_raw_hex_colours_outside_the_palette` を通す）
- 本物の `~/.track` に触れない

## Review Focus

1. `gpui-config.json` も `config.json` も無い初回起動 → 既定値で動き、`gpui-config.json` が作られる（Task 1 で固定）
2. `gpui-config.json` が壊れている（JSON でない）→ 既定値で起動し、`config.json` から写し直さない（Tauri 版の値で上書きしない）（Task 1 で固定）
3. 起動中に macOS の外観が変わる → 「自動」のときだけ追従し、「明るい／暗い」のときは変わらない（Task 2 で固定）
4. 設定ウインドウを開いたまま外観を切り替える → 両方のウインドウが同じ配色になる（Task 3 で固定）
5. 外観の保存に失敗する（書けない）→ 画面は切り替えず、設定画面にエラーを出す（Task 3 で固定）

---

### Task 1: 専用の設定ファイルと移行

**Files:**
- Modify: `gpui/crates/track-core/src/config.rs`
- Modify: `gpui/scripts/prepare-db.sh`（`--force` で `gpui-config.json` も消す）
- Test: `gpui/crates/track-core/src/config_tests.rs`

**Interfaces:**
- Produces:

```rust
pub const FILE: &str = "gpui-config.json";
pub const LEGACY_FILE: &str = "config.json";
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Appearance { #[default] System, Light, Dark }   // JSON "system" | "light" | "dark"
// Config に追加: pub appearance: Appearance
// ConfigPatch に追加: pub appearance: Option<Appearance>
pub fn load_config(data_dir: &Path) -> Config;                 // 既存の名前のまま。gpui-config.json を読む。無ければ移行してから読む
pub(crate) fn migrate_in(data_dir: &Path, dev_root: &Path) -> Result<()>; // gpui-config.json が無いときだけ config.json から写して書く
```

`load_config` は `migrate_in(data_dir, &guard::allowed_root())` を呼んでから読む（テストは `migrate_in` を一時ディレクトリの root で直接呼ぶ）。移行で写すのは今の `Config` の項目すべて（exportDir は `within(dev_root)` でなければ既定）。`gpui-config.json` が壊れていても移行はしない（ファイルがあれば移行済み扱い）。`save_config_in` は `gpui-config.json` に書き、`config.json` には触れない。

- [ ] **Step 1: 失敗するテストを書く**（config_tests.rs）
  - `first_load_copies_config_json_into_gpui_config`: `config.json` に aiProvider=codex, backupKeep=7, exportDir=<root>/x を置く → `migrate_in` 後の `load_config` が同じ値、`gpui-config.json` ができている、`config.json` は変わっていない（中身比較）
  - `migration_drops_export_dir_outside_dev_root`: exportDir が root の外 → 既定（`<data_dir>/exports`）
  - `no_files_gives_defaults_and_creates_gpui_config`（Review Focus 1）
  - `broken_gpui_config_is_not_overwritten_by_legacy`（Review Focus 2）: `gpui-config.json` が `"{"`、`config.json` は codex → `load_config` は既定（apple-intelligence）
  - `save_writes_only_gpui_config`: `save_config_in` で aiProvider を変える → `gpui-config.json` に反映、`config.json` は変わらない
  - `appearance_round_trips`: appearance=dark を保存 → 読むと Dark。不正な値（"blue"）は System
- [ ] **Step 2: 失敗を確認** — Run: `cd gpui && cargo test -p track-core config` / Expected: FAIL（`migrate_in`・`Appearance` が無い）
- [ ] **Step 3: 実装**
- [ ] **Step 4: 通過を確認** — Run: `cd gpui && cargo test --workspace` / Expected: PASS（既存の config・settings テストも。`config.json` を前提にしていたテストは `gpui-config.json` に読み替える）
- [ ] **Step 5: Commit** — `feat(core): GPUI 版専用の設定ファイルに移す`

### Task 2: 暗い配色と明暗の決め方

**Files:**
- Modify: `gpui/crates/track-app/src/ui/theme.rs`
- Modify: `gpui/crates/track-app/src/main.rs`、`testing.rs`（`install_light_theme` を置き換え）
- Modify: `gpui/crates/track-app/src/app.rs`（macOS の外観の変化を購読）
- Test: `gpui/crates/track-app/src/ui/theme.rs`（tests）、`gpui/crates/track-app/src/app_tests.rs`

**Interfaces:**
- Consumes: `track_core::config::Appearance`
- Produces:

```rust
impl Palette { pub fn dark() -> Self; }  // Zed One Dark 寄り
pub fn resolve(setting: Appearance, system_dark: bool, env: Option<&str>) -> bool; // true = dark。env("light"|"dark") が最優先、次に setting、System なら system_dark
pub fn apply_theme(dark: bool, cx: &mut gpui::App);  // Palette を set_global、gpui_component::Theme を Light/Dark にして色を写し、cx.refresh_windows()
```

起動時（main.rs）: `load_config` の appearance・`cx.window_appearance()`・`TRACK_GPUI_APPEARANCE` から `resolve` して `apply_theme`。TrackApp は `cx.observe_window_appearance(window, …)` で macOS の外観変化を受け、その時点の設定で `resolve` し直して `apply_theme`。

- [ ] **Step 1: 失敗するテストを書く**
  - `dark_palette_is_readable`（theme.rs）: Global Constraints の 4 つのコントラスト基準
  - `resolve_prefers_env_then_setting_then_system`: `resolve(System, true, None) == true`、`resolve(Light, true, None) == false`、`resolve(Dark, false, None) == true`、`resolve(Light, false, Some("dark")) == true`、`resolve(System, true, Some("bogus")) == true`（不正な env は無視）
  - `apply_theme_switches_palette_and_component_theme`（app_tests）: `apply_theme(true)` 後 `palette(cx) == Palette::dark()` かつ `Theme::global(cx).mode == ThemeMode::Dark`、`apply_theme(false)` で戻る
- [ ] **Step 2: 失敗を確認** — Run: `cd gpui && cargo test -p track-app theme` / Expected: FAIL
- [ ] **Step 3: 実装**
- [ ] **Step 4: 通過を確認** — Run: `cd gpui && cargo test -p track-app` / Expected: PASS
- [ ] **Step 5: Commit** — `feat(app): 暗い配色と明暗の切り替え`

### Task 3: 設定画面の「外観」

**Files:**
- Create: `gpui/crates/track-app/src/settings/appearance.rs`
- Modify: `gpui/crates/track-app/src/settings/mod.rs`（`SettingsSection::Appearance` を「ビュー」の先頭に、key `appearance`）
- Modify: `gpui/crates/track-app/src/data/mod.rs`（`save_config` はそのまま使う）
- Test: `gpui/crates/track-app/src/settings_tests.rs`

**Interfaces:**
- Consumes: `apply_theme`, `resolve`, `ConfigPatch.appearance`
- Produces: debug selector `settings-nav-appearance`、`appearance-system` / `appearance-light` / `appearance-dark`（選択中が分かる見た目）、`appearance-error`

選ぶと `save_config` → 成功したら `resolve` → `apply_theme`。失敗したら切り替えず `appearance-error` にメッセージ（Review Focus 5）。

- [ ] **Step 1: 失敗するテストを書く**
  - `choosing_dark_saves_and_switches_both_windows`（Review Focus 4）: 設定の「外観」で `appearance-dark` → `gpui-config.json` の appearance が dark、メインと設定ウインドウの両方で `palette(cx) == Palette::dark()`。`appearance-system` に戻すと、テスト環境の外観（`cx.window_appearance()`）どおりになる
  - `system_change_follows_only_when_automatic`（Review Focus 3）: appearance=light のとき、TrackApp の外観変化ハンドラを暗い外観で呼んでも light のまま。system なら dark になる（ハンドラは `TrackApp::on_system_appearance(dark: bool, cx)` として公開し、テストから呼ぶ）
  - `appearance_save_failure_keeps_theme`（Review Focus 5）: DataClient の `fail_ops(&["save_config"])` → `appearance-dark` を押しても palette は変わらず `appearance-error` が出る
- [ ] **Step 2: 失敗を確認** — Run: `cd gpui && cargo test -p track-app settings_tests` / Expected: FAIL
- [ ] **Step 3: 実装**
- [ ] **Step 4: 通過を確認** — Run: `cd gpui && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` / Expected: PASS、警告なし
- [ ] **Step 5: Commit** — `feat(app): 設定画面に外観を追加`

### Task 4: 実地確認とドキュメント

**Files:**
- Modify: `gpui/README.md`、`gpui/NOTES.md`

- [ ] **Step 1: コピー DB で起動** — `TRACK_GPUI_APPEARANCE=dark TRACK_DATA_DIR="$HOME/.track-gpui"` でアプリを起動し、カレンダー・レポート・設定をウインドウだけ撮影して確認（撮影は commit しない）。`~/.track-gpui/gpui-config.json` ができ、`config.json` の値（AI・バックアップ）が写っていることを確かめる
- [ ] **Step 2: ドキュメント** — README の「設定」節を `gpui-config.json` に書き換え、外観と `TRACK_GPUI_APPEARANCE` を追記。NOTES に暗い配色の所感
- [ ] **Step 3: 全テスト** — Run: `cd gpui && cargo test --workspace` / Expected: PASS
- [ ] **Step 4: Commit** — `docs(gpui): 暗い配色と専用の設定ファイル`
