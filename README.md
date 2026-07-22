# Track — 工数管理アプリ

Vite + React / Cloudflare Workers + D1 構成の工数管理アプリ。

## セットアップ

### 1. 依存パッケージのインストール

```bash
npm install
```

### 2. Prismaクライアントの生成

```bash
npx prisma generate
```

> `DATABASE_URL` が必要なため `.env` ファイルを先に作成すること（下記参照）。

### 3. 環境変数の設定

`.env` ファイルをプロジェクトルートに作成：

```env
DATABASE_URL=file:./prisma/dev.db
```

> `DATABASE_URL` はPrisma CLIがローカルで使用するもの。アプリ実行時はCloudflare D1を使うため不要。

### 4. ローカルDBにマイグレーションを適用

```bash
npm run db:migrate:local
```

### 5. 開発サーバーの起動

ターミナルを2つ使って同時に起動：

```bash
# フロントエンド (Vite)
npm run dev

# バックエンド (Cloudflare Workers)
npm run dev:worker
```

フロントエンド: http://localhost:5173  
バックエンド: http://localhost:8787

---

## コマンド一覧

| コマンド | 内容 |
|---|---|
| `npm run dev` | Vite devサーバー起動 |
| `npm run local` | Bunローカルサーバー起動 |
| `npm run tauri:dev` | Tauriデスクトップ版を開発起動 |
| `npm run tauri:build` | Bun sidecar同梱のmacOS `.app` をビルド |
| `npm run tauri:dmg` | 配布用DMGをビルド |
| `npm run dev:worker` | Wrangler devサーバー起動 |
| `npm run db:migrate:local` | ローカルD1にマイグレーション適用 |
| `npm run db:migrate:remote` | 本番D1にマイグレーション適用 |
| `npx prisma generate` | Prismaクライアント再生成 |
| `npm run deploy` | ビルド＆Cloudflareへデプロイ |

デスクトップ版のビルドには、BunとRustが必要です。生成された`.app`は
`src-tauri/target/release/bundle/macos/Track.app`に出力されます。Bun sidecar、
フロントエンド、SQLiteマイグレーションを同梱するため、利用端末へのNode/Bunの
インストールは不要です。

## デプロイ

```bash
npx wrangler login   # 初回のみ
npm run deploy
```

本番DBへのマイグレーション適用：

```bash
npm run db:migrate:remote
```
