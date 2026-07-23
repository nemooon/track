# Track — 開発ガイド

## 技術スタック

- フロントエンド: Vite + React 19 + Tailwind CSS v4 + Zustand + TanStack Query
- ローカル API: Bun + Hono
- DB: SQLite + Prisma + libSQL アダプタ
- デスクトップ: Tauri 2（Bun コンパイル済み sidecar を同梱）

## コード配置

- `src/client/` — React SPA。ブラウザ固有のコードはここに置く
- `src/server/` — Bun API。`routes/`、`db/`、`fixtures/` に分ける
- `src/shared/` — client/server の両方から使う型、日付処理、Zod スキーマ
- `database/migrations/` — 起動時とリストア後に適用する手書き SQL
- `database/schema.prisma` — Prisma スキーマ
- `src-tauri/` — Tauri のRustコード、バンドル設定、`scripts/`内のsidecarビルド処理

import alias は `@client/*`、`@server/*`、`@shared/*` を使う。

## 開発コマンド

- `npm run local` — Bun API を起動
- `npm run dev` — Vite 開発サーバーを起動
- `npm run tauri:dev` — Tauri アプリを開発起動
- `npm run build:desktop` — SPA と Bun sidecar をビルド
- `npm run tauri:build` — macOS `.app` を生成
- `npm run tauri:dmg` — DMG を生成

実行時 DB は既定で `~/.track/track.db`。Tauri はresourceディレクトリと待受ポートを環境変数でsidecarに渡す。利用端末にNode.jsやBunは不要。

本番Tauriアプリは空きloopbackポートを動的に選び、Single Instanceプラグインで二重起動を防ぐ。sidecar起動失敗はDialogプラグインでネイティブ表示する。localhostで配信される画面には、バックアップ先フォルダを選ぶ`dialog:allow-open`、タイトルバーをドラッグする`core:window:allow-start-dragging`、ネイティブメニューイベントを受ける`core:event:allow-listen`だけを個別に付与する。

デスクトップUIは左サイドバーを持たない。共通ヘッダーは左にカレンダー・レポート切替、中央に`PageHeaderPortal`で差し込むページ固有操作、右に`HeaderDateNavigation`とアプリメニューを置く。ヘッダーの操作部品以外を押した場合は`startDragging`を呼び、ウインドウのドラッグ領域として扱う。macOSのネイティブメニューは標準動作とショートカットを維持した日本語メニューとしてRust側で構築する。画面切替は`⌘1`（カレンダー）・`⌘2`（レポート）、表示期間の移動は`⌘[`・`⌘]`、今日への移動は`⌘T`、カレンダーの時間軸拡大縮小は`⌘+`・`⌘-`で行え、Rustのメニューイベントとブラウザのキーボードイベントを共通のReactナビゲーションへ接続する。設定オーバーレイの開閉はURLではなく`AppUiContext`が所有し、macOSのTrackメニューまたは`⌘,`からTauriイベントで開く。クライアント・プロジェクト・タグ管理も設定画面に含める。
