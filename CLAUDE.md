# Track — 開発ガイド

## 技術スタック

- アプリ: Rust + GPUI（gpui-pre）+ gpui-component
- DB: SQLite（rusqlite）。サーバーは持たず、アプリと CLI が直接読み書きする
- 以前の Tauri 版（Vite + React + Bun + Prisma）は `tauri` ブランチとタグ `v0.7.4` にだけ残っている

## コード配置

- `crates/track-core/` — DB・集計・検証・AI 登録・`prepare`・AI 生成・バックアップ・連携のインストール
- `crates/track-cli/` — `track-cli`（AI スキルから呼ぶ CLI。JSON で答える）
- `crates/track-app/` — GPUI アプリ（`track-gpui`）。`calendar/`、`reports/`、`settings/`、`ui/`
- `database/migrations/` — 手書き SQL。`track-core/src/migrate.rs` が `include_str!` で埋め込み、起動時とリストア後に当てる
- `integrations/` — Codex スキルと Claude Code コマンドの元の文面（`track-core/src/integrations.rs` が埋め込む）
- `helpers/apple-intelligence/` — Apple Intelligence で生成するためのヘルパーアプリ（Swift）
- `packaging/` — `.app` の `Info.plist` とアイコン（`icon.svg` から `scripts/build-icon.sh` で作る）
- `scripts/` — `package.sh`（アルファ版を作る）、`prepare-db.sh`（コピー DB を作る）、`winshot.sh`（アプリのウインドウだけを撮る）、`build-ai-helper.sh`（ヘルパーをアプリの隣の `target/debug/` にビルド）、`build-icon.sh`（`icon.py` で描いたアイコンを `packaging/` の icns と PNG にする）
- `docs/superpowers/` — 設計書と実装計画

## 開発コマンド

- `scripts/prepare-db.sh` — `~/.track` の DB を `~/.track-gpui` にコピーする（`--force` で作り直し）
- `cargo build --workspace` — アプリと `track-cli` を同じフォルダにビルド
- `TRACK_DATA_DIR="$HOME/.track-gpui" cargo run` — アプリを起動（`~` ではなく `$HOME` を使う）
- `TRACK_GPUI_UPDATE_PREVIEW=9.9.9` を付けて起動すると、更新ウインドウが見本の新しい版を出す（開発版だけ。アップデートは何もせず成功にする）
- `cargo test --workspace` — テスト。`cargo test` だけではアプリのバイナリは作り直されないので、実画面で確かめる前に `cargo build -p track-app`
- `cargo clippy --workspace --all-targets` — 警告ゼロを保つ（`--features track-core/real-data` を付けた形でも）
- `scripts/package.sh` — アルファ版の `Track.app` と DMG を `target/package/` に作る。これだけが開発用ガードを外して本物の `~/.track` を開く

## 守ること

- 開発版は `~/.track-gpui` のコピー DB だけを扱う（開発用ガード）。本物の `~/.track` と iCloud Drive のバックアップには触れない。パッケージ版を試すときも `TRACK_DATA_DIR="$HOME/.track-gpui"` を付けて起動する
- 色は `ui/theme.rs` の `Palette` を意味の名前で引く（`palette(cx)`）。生の色は書かない（テストで確かめている）
- UI のテストは GPUI の TestAppContext で、`debug_selector` を付けた要素の位置を確かめる（`src/testing.rs`）
- 確認や選択は押した部品からのポップオーバーで出し、行の下に開く UI は作らない

## 画面

ヘッダーは左にカレンダー・レポートの下線タブ（カレンダーのアイコンに今日の日）、右に設定の歯車。背景のドット模様はデータが変わるたびにきらめく（`ui/header_dots.rs`、合図は `DataClient::changes`）。
画面切替は `⌘1`・`⌘2`、期間の移動は `⌘[`・`⌘]`、今日へは `⌘T`、時間軸の拡大縮小は `⌘+`・`⌘-`、設定は `⌘,`（別ウインドウ）。
