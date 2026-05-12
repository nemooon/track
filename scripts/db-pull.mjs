#!/usr/bin/env node
// 本番 D1 のデータをローカル D1 へ同期する。
// パスキーはホスト名 (RP ID) に紐づくため、本番のものはローカルでは使えない。
// そこで pull 前にローカルの Credential を email 付きで退避し、import 後に
// 同じ email の本番ユーザーへ付け替える。
import { execSync } from "node:child_process";
import { rmSync, mkdirSync, writeFileSync } from "node:fs";

const DUMP = ".wrangler/dump.sql";
const POST = ".wrangler/post.sql";

const sh = (cmd) => execSync(cmd, { stdio: "inherit" });
const cap = (cmd) =>
  execSync(cmd, { stdio: ["inherit", "pipe", "inherit"] }).toString();
const q = (v) =>
  v == null ? "NULL" : `'${String(v).replace(/'/g, "''")}'`;

mkdirSync(".wrangler", { recursive: true });

let saved = [];
try {
  const out = cap(
    `npx wrangler d1 execute track --local --json --command "SELECT c.id, c.credentialId, c.publicKey, c.counter, c.transports, c.createdAt, u.email FROM Credential c JOIN User u ON c.userId = u.id"`,
  );
  saved = JSON.parse(out)?.[0]?.results ?? [];
  console.log(`==> 既存ローカルパスキー ${saved.length} 件を退避`);
} catch {
  console.log("==> 退避するローカルパスキーなし");
}

console.log("==> リモート D1 からダンプ");
sh(`npx wrangler d1 export track --remote --output=${DUMP}`);

console.log("==> ローカル D1 を初期化");
rmSync(".wrangler/state/v3/d1", { recursive: true, force: true });

console.log("==> ダンプを流し込み");
sh(`npx wrangler d1 execute track --local --file=${DUMP}`);

if (saved.length > 0) {
  const lines = [];
  for (const c of saved) {
    lines.push(
      `DELETE FROM Credential WHERE userId IN (SELECT id FROM User WHERE email = ${q(c.email)});`,
      `INSERT INTO Credential (id, userId, credentialId, publicKey, counter, transports, createdAt) ` +
        `SELECT ${q(c.id)}, id, ${q(c.credentialId)}, ${q(c.publicKey)}, ${Number(c.counter ?? 0)}, ${q(c.transports)}, ${q(c.createdAt)} ` +
        `FROM User WHERE email = ${q(c.email)};`,
    );
  }
  writeFileSync(POST, lines.join("\n") + "\n");
  console.log("==> ローカルパスキーを本番ユーザーへ付け替え");
  sh(`npx wrangler d1 execute track --local --file=${POST}`);
}

console.log("==> 完了");
