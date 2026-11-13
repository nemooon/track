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

### AI からの編集と取り消し

スキルには記録に加えて、登録済み工数の修正・付け替え・削除と、クライアント・プロジェクト・タグの作成・変更の手順が入る（GPUI 版だけの節。共有の `integrations/` は変えない）。

```bash
track-cli search --project-id <ID> --from <ISO> --to <ISO>      # 対象を集める
track-cli update --id <ID> --id <ID> --project-id <ID> --source codex
track-cli history                                               # AI の操作一覧
track-cli undo --id <操作ID>                                    # 取り消し
```

- 確認が要る操作（`--confirmed` が無いと `confirmation_required`）: 工数 10 件以上の変更、工数の削除、クライアント・プロジェクトの作成、タグの削除
- 書き込みは 1 コマンドごとに `AiOperation` テーブル（`database/migrations/0016_ai_operation.sql`）へ変更前後の行と一緒に残る。直近 100 件・30 日を保持
- 取り消しは、その操作のあとで行が変わっていれば `undo_conflict` で止まる（部分的には戻さない）
- アプリは AI の操作を検知すると本体の上に「AI が N 件変更しました［元に戻す］」を出す。表示中に続けて届いた操作（プロジェクト作成と複数件の記録など）はまとめられ、［元に戻す］で新しい順に全部戻す（戻せない操作があればそこで止まる）。×で閉じるとまとまりが区切られる

## 設定

`⌘,`、メニュー「Track → 設定…」、ヘッダー右の歯車で別ウインドウの設定を開く。項目は Tauri 版と同じ
（カレンダー／レポート（出力フォーマット・週報テンプレート）／プロジェクト／AI／バックアップと復元。メモは無し）。
勤務設定・出力フォーマットは DB の `Settings`、AI とバックアップは `config.json` に、Tauri 版と同じ形式で保存する
（`config.json` の知らない項目は消さない）。

自動バックアップはアプリが起動時と 1 時間ごとに確認し、`backupIntervalHours` 経っていれば `VACUUM INTO` で
`track-auto-*.db` を保存先へ書く。開発版では保存先・復元元を `~/.track-gpui` の中に限り、iCloud Drive の候補も出さない
（本物のバックアップがそこにあるため）。

AI 生成（週報・カスタム出力）は `config.json` の `aiProvider` に従い、Codex CLI・カスタムコマンド・
Apple Intelligence ヘルパー（`/Applications/Track.app` 同梱）をアプリから直接起動する。

## 開発用

- `scripts/winshot.sh out.png` — アプリのウインドウだけをスクリーンショットする
- `TRACK_GPUI_PAGE=reports cargo run` — レポート画面から起動する
- `TRACK_GPUI_WINDOW=1440x900` — 起動時のウインドウサイズ
- `TRACK_GPUI_SETTINGS=<calendar|reports|projects|ai|backup>` — 起動時に設定ウインドウをその項目で開く（`winshot.sh` はどちらかのウインドウを撮るので、設定だけ撮るときはウインドウ ID を指定する）
