# Track GPUI 版（開発中）

カレンダーとレポートを GPUI（Zed の Rust UI フレームワーク）で作り直したもの。サーバーを持たず、
アプリと CLI が共通の `track-core` で SQLite を直接読み書きする。
設計: `docs/superpowers/specs/2026-11-12-gpui-native-design.md`

## 構成

- `crates/track-core` — DB・集計・AI 登録・`prepare`・AI 生成プロバイダ・連携のインストール
- `crates/track-cli` — `track-cli`（`src/cli/index.ts` と同じサブコマンドと JSON）
- `crates/track-app` — GPUI アプリ（`track-gpui`）

## 必要なもの

- Rust 1.98.1（`rust-toolchain.toml` で固定。rustup が自動で入れる）
- Xcode 本体は不要（`runtime_shaders` でシェーダーを実行時にコンパイルする）
- `sqlite3` と `python3`（`prepare-db.sh` が使う。macOS の Command Line Tools に含まれる）
- Bun や API サーバーは不要

## 起動

開発版はコピーした DB（`$HOME/.track-gpui`）だけを扱う。アプリも CLI も、ほかの場所を指定するとエラーで止まる。

```bash
gpui/scripts/prepare-db.sh            # ~/.track-gpui に DB と config.json をコピー（--force で作り直し）
cd gpui && cargo build --workspace    # アプリと track-cli を同じフォルダにビルド
TRACK_DATA_DIR="$HOME/.track-gpui" cargo run
```

`TRACK_DATA_DIR=~/...` と書くと zsh では `env` の引数の `~` が展開されない。必ず `$HOME` を使う。

アプリは起動時に `~/.track-gpui/runtime.json` へ `cliPath`（アプリと同じフォルダの `track-cli`）を書く。
CLI はアプリを起動していなくても使えるので、終了時にも消さない。
別の接続（CLI や AI）が DB に書き込むと、アプリは 1 秒以内に検知して表示を読み直す。

## AI 連携（開発用）

メニューの「Track → AI 連携をインストール…（開発用）」で、Codex / Claude Code / 両方を選んでインストールする。
本物の `track` スキルを上書きしないよう、開発中は別名で入る。

- Codex: `~/.codex/skills/track-dev/`（`$track-dev` で呼ぶ）
- Claude Code: `~/.claude/commands/track-dev.md`（`/track-dev` で呼ぶ）

文面は `integrations/` のものを元に、`$HOME/.track-gpui/runtime.json` を読み、`TRACK_DATA_DIR="$HOME/.track-gpui"` を付けて CLI を呼ぶよう書き換えてある。
本採用時は `ui/integrations_dialog.rs` の `DEV_NAMES` を外し、本来の名前に戻す。

AI 生成（週報・カスタム出力）は `config.json` の `aiProvider` に従い、Codex CLI・カスタムコマンド・
Apple Intelligence ヘルパー（`/Applications/Track.app` 同梱）をアプリから直接起動する。

## 開発用

- `scripts/winshot.sh out.png` — アプリのウインドウだけをスクリーンショットする
- `TRACK_GPUI_PAGE=reports cargo run` — レポート画面から起動する
- `TRACK_GPUI_WINDOW=1440x900` — 起動時のウインドウサイズ
