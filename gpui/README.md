# Track GPUI 試作

カレンダーとレポートを GPUI（Zed の Rust UI フレームワーク）で作り直す試作。既存の Bun API に HTTP でつなぐ。
設計: `docs/superpowers/specs/2026-11-12-gpui-prototype-design.md`

## 必要なもの

- Rust 1.98.1（`rust-toolchain.toml` で固定。rustup が自動で入れる）
- Xcode 本体は不要（`runtime_shaders` でシェーダーを実行時にコンパイルする）
- Bun（API サーバー用、リポジトリ直下の `npm run local`）
- `sqlite3` と `python3`（`prepare-db.sh` が使う。macOS の Command Line Tools に含まれる）

## 起動

本物の `~/.track/` を触らないよう、コピーした DB を使う。

```bash
gpui/scripts/prepare-db.sh            # ~/.track-gpui に DB と config.json をコピー（--force で作り直し）
env -u TRACK_EXPORT_DIR TRACK_DATA_DIR="$HOME/.track-gpui" npm run local   # 別端末で API を起動（127.0.0.1:8787）
cd gpui && cargo run
```

`prepare-db.sh` はコピーした `config.json` の `exportDir` を `~/.track-gpui/exports` に書き換える。
API サーバーの自動バックアップが本物のバックアップを上書き・削除しないようにするため。
環境変数 `TRACK_EXPORT_DIR` は `exportDir` より優先されるので、起動コマンドで `env -u` で外している。

`TRACK_DATA_DIR=~/...` と書くと zsh では `env` の引数の `~` が展開されず、カレントディレクトリに `~` フォルダができて空の DB が作られる。必ず `$HOME` を使う。

試作アプリは起動時に API の `/health` が返す DB パスを確認し、ホーム直下の `.track-gpui/` を指す絶対パス以外なら読み込みを止めてエラーを出す。
通常の `npm run local`（本物の `~/.track`）が 8787 番で動いたままでも、誤って本物を書き換えない。

接続先のポートは環境変数 `TRACK_PORT` で変えられる。

## 開発用

- `scripts/winshot.sh out.png` — 試作アプリのウインドウだけをスクリーンショットする
- `TRACK_GPUI_PAGE=reports cargo run` — レポート画面から起動する
