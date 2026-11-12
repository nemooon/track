# Track GPUI ネイティブ版 — 設計

日付: 2026-11-12 / ブランチ: `gpui-native`（`gpui-prototype` の続き）
前段: `docs/superpowers/specs/2026-11-12-gpui-prototype-design.md`、`gpui/NOTES.md`

## 目的と背景

GPUI 試作は「React 版と同じものが作れるか」の検証として、Bun の HTTP API を叩く Web アプリの作りをそのまま移植した。結果、応答速度は Tauri 版とほぼ同じで、差は操作中の負荷とメモリだった。次の段階として、**Web の作りの移植ではなく GPUI に向いた作り**に作り変え、手触りと保守性がどう変わるかを確かめる。採用するかは引き続き出来上がり次第。

利用者にとって Track は「**レポートを出すために記録する**」道具である。
- 入力はほぼ AI（Codex。利用者によっては Claude Code）に CLI 経由でさせている
- 毎週: レポート画面で**案件ごとの1週間の合計**を見て、完了表に**10進数の時間（1時間半 → 1.5）**で転記する（数字は手で微調整する）
- 月末: 案件によって**カスタム出力をコピー&ペースト**して稼働報告書を作る
- カレンダーは毎日は触らないが、AI が入れた記録を確認・修正する中核の画面

## ゴール / 非ゴール / アンチゴール

**ゴール**
1. Rust の中核クレート（DB・検証・集計・AI 登録・AI 生成）を作り、GPUI アプリと Rust 製 CLI が共有する。どちらも SQLite を直接読み書きし、HTTP サーバーは置かない
2. 新 CLI で、既存の Codex スキル（`integrations/codex/track/SKILL.md`）と Claude Code コマンド（`integrations/claude/track.md`）がそのまま、またはごく小さな変更で動く。**両方とも対応し、利用者が選べる**
3. カレンダーを、試作と同等以上の完成度で、プロセス内のデータで作り直す
4. レポートを作り直し、毎週の転記（案件ごとの週合計を10進数の時間で読む）をしやすくし、月末のカスタム出力（AI あり／なし）を今と同じように使えるようにする
5. CLI や別プロセス（Tauri 版を含む）による DB の変更を、アプリが検知して画面に反映する

**非ゴール**
- Tauri 版の画面に、Rust の CLI や GPUI 版による DB の変更をリアルタイムで反映すること（Claude の判断）。開発中は別の DB を使うので起きない。本採用を決めるときの課題として残す
- 報告用の時間を調整・記録する機能（利用者が不要と判断）
- 設定画面（クライアント・プロジェクト・タグ・出力形式・週報テンプレートの管理）、メモ、データの移行・バックアップ画面、アップデート、`.app` 化・配布。これらは同じ DB を使う Tauri 版で行える
- Tauri 版の置き換え、同時リリース

**アンチゴール（こうなったら失敗）**
- 既存の DB を Tauri 版が読めなくなる。スキーマ・日時の保存形式・ID・`_local_migrations` を互換に保つ
- Codex スキルの登録が止まる、または同じ登録が二重になる（`requestId` による冪等性が壊れる）
- 開発中に本物の `~/.track` を書き換える（試作と同じく、コピー DB と DB パス確認で防ぐ）

## 決まっていること（利用者の判断）

- AI からのアクセスは **A 案**：CLI が中核クレートを使って DB を直接読み書きする。アプリの起動を前提にしない
- Claude Code 連携も作る（本人は Codex 中心だが、利用者に選ばせる）
- レポートは見やすさを優先し、報告用の調整機能は作らない
- `gpui-prototype` から新しいブランチを切って作り込む

## 構成（Claude の判断）

Cargo ワークスペースを `gpui/` に置き、次の3クレートに分ける。

| クレート | 役割 |
|---|---|
| `track-core` | DB 接続・マイグレーション・リポジトリ（工数・プロジェクト・クライアント・タグ・設定・AI 登録）、検証、レポート集計、カスタム出力・週報のロジック（試作の `core/report` を移す）、AI 生成プロバイダ、会話ログからの候補作成（`prepare`）。GPUI に依存しない |
| `track-cli` | 既存 CLI と同じサブコマンド・フラグ・JSON 出力・エラー形式 |
| `track-app` | GPUI アプリ。試作の画面部品を流用し、データ取得を `track-core` 直接呼び出しに置き換える |

### データ層の互換条件（必ず守る）
- DB ファイル: `$TRACK_DATA_DIR/track.db`（既定 `~/.track/track.db`）。WAL モード
- マイグレーション: `database/migrations/*.sql` を `include_str!` で埋め込み、同じ `_local_migrations(name, appliedAt)` 表で管理し、同じ順序・同じ手順（`foreign_keys=OFF`、`legacy_alter_table=ON`、各ファイルを BEGIN/COMMIT、最後に `foreign_key_check`）で適用する。Tauri 版と同じ DB に対してどちらが先に起動しても壊れない
- 日時は TEXT `YYYY-MM-DDTHH:mm:ss.SSS+00:00`、真偽値は INTEGER 0/1、ID は cuid 形式（Prisma と同じ形）。`TimeEntry.updatedAt` は書き込み時に必ず設定する
- `Settings` が無ければ既定値（workStart 600, workEnd 1110, workDays "1,2,3,4,5"、既定の週報テンプレート）で1行作る。`reportCopyFormats` が空・不正なら既定の「標準」形式を使う（`src/server/routes/settings.ts:12-51` と同じ）

- **マイグレーションの食い違い**（Claude の判断）: DB の `_local_migrations` に、このバイナリが知らない名前があれば（DB の方が新しい）、アプリは読み取り専用で開いて警告し、CLI の `create` は `schema_too_new` で拒否する。バイナリの方が新しければ、足りない分を適用する

### 業務ルールの互換条件（既存サーバーと同じ結果にする）
- 工数作成・更新: `src/shared/validators.ts` の制約、15分への丸め（AI 登録時は丸めずに15分ちょうどでなければ `invalid_time_step`）、同じ日の規則（0:00 終了は前日扱い）、タグの重複除去
- AI 登録: `requestId` による再送の冪等性。`payloadHash` は既存と**同じキー順・同じ日時表記**で計算し、Tauri 版で登録したものの再送も正しく判定する（`src/server/routes/entries.ts:76-163`）。重複・重なり（`exact_duplicate`、`overlap_requires_confirmation`、`--allow-overlap`）、`request_id_conflict`、`registered_entry_deleted`、`AiRegistration` 行の作成をトランザクション内で行う
- レポート集計: 範囲（週は日曜始まり、月はその月）、範囲内への切り詰め、休憩時間の按分、絞り込み（`__none__`、タグはエントリとプロジェクトの OR）、タグ集計の規則（`src/server/routes/reports.ts`）
- タイムゾーン: CLI の同日判定と `prepare` は JST 固定（既存 CLI と同じ）。アプリと集計はローカル時刻（既存サーバーと同じ）

### CLI（`track-cli`）
- サブコマンド: `status`、`projects`、`list --from --to`、`prepare`（day / session）、`create`、`help`。フラグ・既定値・JSON 出力のフィールド名・エラーコードは既存 `src/cli/index.ts` と同じ。成功時も失敗時も stdout に JSON、失敗時は終了コード 1
- `status` は `{connected:true, dbPath, projectCount}` を返す（`apiBase` の代わりに `dbPath`。スキルは参照していない）
- `prepare` のアルゴリズム（ログの場所、`subagents` の除外、15分単位の窓、既存工数と登録履歴の除外、`parallelWith`、`totalCandidateMinutes`、メッセージの抜粋規則）は既存と同じ。既存の `src/cli/index.test.ts` のケースを Rust に移して同じ結果を確かめる
- CLI の場所はアプリが `runtime.json` の `cliPath` に書く（スキルはこれで CLI を探す）。DB の場所は `TRACK_DATA_DIR`（既定 `~/.track`）
- **アプリが起動していなくても CLI は使える**（Claude の判断）。そのため GPUI アプリは `runtime.json` を終了時に消さない（`cliPath` はアプリ内の固定パスで、アプリを閉じても有効）。`pid` と `startedAt` は参考情報として残る
- スキルとコマンドの文面は、新 CLI に合わせて小さく直してよい（Claude の判断）。対象は、HTTP 前提のエラー（`network_error` など）と「Track を起動してください」の案内。`prepare` と `create` の使い方と出力の読み方は変えない

### アプリ（`track-app`）
- **状態**: 表示中の範囲の工数、参照データ（プロジェクト・クライアント・タグ・設定）をメモリ上に持つ。DB への書き込みはバックグラウンドで行い、終わったら状態を更新する。失敗時は元に戻して通知する（ローカル DB なので失敗はまれ）
- **外部変更の検知**: 1秒ごとに `PRAGMA data_version` を確認し、変わっていたら表示中のデータを読み直す。CLI や Tauri 版による登録が数秒以内に反映される
- **runtime.json**: 起動時に `{pid, cliPath, dbPath, startedAt}` を書き、終了時に自分の `pid` のときだけ消す（既存の `src/server/runtime.ts` と同じ作法）。`apiBase` は書かない
- **AI 連携のインストール**: Codex スキルと Claude Code コマンドを、既存の Tauri 版と同じ場所・同じ上書き規則でインストールするメニュー項目を持つ（`src-tauri/src/lib.rs:33-210` の規則を移す）。利用者がどちらか、または両方を選べる
- **カレンダー**: 試作の機能一覧をすべて保つ。データ取得を `track-core` に置き換え、取得待ちの表示や接続エラー画面は不要になる
- **レポート**:
  - 時間の表示形式を「1h 30m」と「1.5」で切り替えられる（既定は 1.5、選択は保存）。10進数は小数第2位まで（`1.33`）。丸め単位（0.25 / 0.5）は付けない（必要なら後で）
  - **週の転記を最短にする**: 週表示のとき「週の合計をコピー」ボタンで、**プロジェクトごと**（利用者の判断。完了表の「案件」はプロジェクト）の週合計を `プロジェクト名<TAB>1.5` の行で、見出しなしの TSV としてコピーする。表示中の絞り込みに従う。プロジェクトなしの工数は「プロジェクトなし」の行にする。各行はその行の実績を小数第2位で丸めた値で、行の和を合計に合わせる調整はしない（Claude の判断。完了表へは利用者が手で微調整して入れるため）
  - 月末のカスタム出力（AI あり／なし）、AI 週報、絞り込み、明細、ドーナツは試作と同等
- **AI 生成プロバイダ**: `codex`（`codex exec --experimental-json` を直接起動して JSONL イベントを読む。SDK は使わない）、`custom-command`、`apple-intelligence`（インストール済み Track.app 内の `TrackAIHelper.app` を利用）。設定は `~/.track/config.json` の既存フィールドを読む

### 開発中の安全策（試作から引き継ぎ）
- 開発中はコピー DB（`$HOME/.track-gpui`、`gpui/scripts/prepare-db.sh`）だけを使う。アプリ・CLI とも、`TRACK_DATA_DIR` が `$HOME/.track-gpui` を指す絶対パスでなければ、書き込みを伴う起動を拒否する開発用ガードを付ける（本採用時に外す）
- 開発中に本物の Codex スキル（`~/.codex/skills/track`）や Claude Code コマンド（`~/.claude/commands/track.md`）を上書きしない（Claude の判断）。開発中のアプリの「AI 連携をインストール」は、**別名の開発用**（Codex: `~/.codex/skills/track-dev/`、Claude Code: `~/.claude/commands/track-dev.md`）としてインストールし、`$HOME/.track-gpui/runtime.json` を読むように書き換えた文面を使う。本採用時に本来の名前へ戻す

## 進め方

1. **中核クレートと CLI**: DB・マイグレーション・リポジトリ・検証・AI 登録・`prepare` を作り、既存 CLI と同じ出力になることをテストで確かめる。コピー DB で Codex スキルの登録が通ることを確認する
2. **カレンダー**: 試作の画面を `track-core` に載せ替える。外部変更の検知を入れる
3. **レポート**: 集計を `track-core` に移し、10進数表示と週の転記コピーを足す。カスタム出力・週報の AI 生成をプロバイダ込みで動かす
4. **AI 連携のインストール**と `runtime.json`
5. 試作版（HTTP 版）との比較計測を `gpui/bench` で再実行し、`gpui/NOTES.md` に追記する

## 完了時の成果物

- `gpui/` の Cargo ワークスペース（`track-core`、`track-cli`、`track-app`）
- コピー DB で、アプリと CLI（Codex スキル経由を含む）が動く状態
- `gpui/NOTES.md` への追記（作りを変えて何が変わったか、計測値、本採用に残る作業）

## 実装者に任せること

- `track-core` 内のモジュール分け、SQLite ライブラリの選択（`rusqlite` を想定）、cuid の生成方法
- 試作コードのどこを流用・削除するか（HTTP クライアント、SSE、接続エラー画面は不要になる）
- レポートの週転記ボタンの置き場所と見た目
