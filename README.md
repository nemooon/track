# Track — 工数管理アプリ

React の画面と Bun のローカル API を Tauri にまとめた、macOS 向けの工数管理アプリです。SQLite データベースは利用者のホームディレクトリに保存します。

## ディレクトリ構成

```text
track/
├── src/
│   ├── client/       # React SPA（画面・コンポーネント・ブラウザ側処理）
│   ├── server/       # Bun + Hono API（ルート・DB・外部予定表）
│   └── shared/       # client/server 共通の型・日付処理・入力検証
├── database/
│   ├── migrations/   # 起動時に順番に適用する手書き SQL
│   └── schema.prisma # Prisma スキーマ
└── src-tauri/        # Tauri の Rustコード・設定・sidecarビルド処理
```

## セットアップ

必要なものは Node.js、Bun、Rust です。

```bash
npm install
```

Prisma CLI を使う場合は、プロジェクトルートに `.env` を作成します。

```env
DATABASE_URL=file:./database/dev.db
```

その後、クライアントを生成します。

```bash
npx prisma generate
```

## 開発

ブラウザで開発するときは、ターミナルを2つ使います。

```bash
# Bun API（http://127.0.0.1:8787）
npm run local

# Vite（http://localhost:5173）
npm run dev
```

Vite は `/api` を Bun API にプロキシします。ローカル実行時のデータは既定で `~/.track/track.db` に保存されます。

デスクトップアプリとして開発する場合は次を使います。

```bash
npm run tauri:dev
```

## ビルド

```bash
# Bun sidecar を同梱した macOS .app
npm run tauri:build

# 配布用 DMG
npm run tauri:dmg
```

`.app` は `src-tauri/target/release/bundle/macos/Track.app` に生成されます。Bun の実行環境、React のビルド成果物、SQLite マイグレーションを同梱するため、利用端末への Node.js や Bun のインストールは不要です。

本番アプリは空いているlocalhostポートを起動時に選びます。二度起動した場合は新しいプロセスを増やさず、既存ウィンドウを前面へ戻します。sidecarやDBの起動に失敗した場合は、macOSのエラーダイアログに原因を表示します。

バックアップ先は設定画面の「選択」ボタンからmacOS標準のフォルダ選択ダイアログで指定できます。ブラウザ開発時は従来どおりパスを直接入力します。

## コマンド

| コマンド | 内容 |
|---|---|
| `npm run dev` | Vite 開発サーバーを起動 |
| `npm run local` | Bun API を起動 |
| `npm run build` | React SPA をビルド |
| `npm run build:sidecar` | Bun API をTauri用の単一実行ファイルへコンパイル |
| `npm run build:desktop` | SPA と sidecar をビルド |
| `npm run tauri:dev` | Tauri アプリを開発起動 |
| `npm run tauri:build` | macOS `.app` をビルド |
| `npm run tauri:dmg` | DMG をビルド |
