import { afterEach, describe, expect, test } from "bun:test";
import { randomUUID } from "node:crypto";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { Hono } from "hono";
import { PrismaClient } from "@prisma/client";
import { PrismaLibSql } from "@prisma/adapter-libsql";
import { runMigrations } from "../db/migrate";
import { entries } from "./entries";
import type { Env } from "../types";

const fixtures: { dir: string; prisma: PrismaClient }[] = [];
afterEach(async () => {
  for (const { dir, prisma } of fixtures.splice(0)) {
    await prisma.$disconnect();
    rmSync(dir, { recursive: true, force: true });
  }
});
function setup() {
  const dir = mkdtempSync(path.join(tmpdir(), "track-entry-"));
  const dbPath = path.join(dir, "test.db");
  runMigrations(dbPath, path.resolve("database/migrations"));
  // 二度適用しても新旧マイグレーションを重複実行しない。
  expect(runMigrations(dbPath, path.resolve("database/migrations"))).toEqual([]);
  const prisma = new PrismaClient({ adapter: new PrismaLibSql({ url: `file:${dbPath}` }) });
  fixtures.push({ dir, prisma });
  const app = new Hono<{ Bindings: Env }>();
  app.use("*", async (c, next) => { c.env = { DB: prisma } as Env; await next(); });
  app.onError(() => new Response("test database error", { status: 500 }));
  app.get("/api/projects", (c) => c.json([]));
  app.route("/api/entries", entries);
  return { app, prisma };
}
function payload() {
  return { start: "2026-10-01T10:00:00+09:00", end: "2026-10-01T10:30:00+09:00", title: "調査",
    aiRegistration: { requestId: randomUUID(), source: "codex", sessionId: "session-a" } };
}
function post(app: ReturnType<typeof setup>["app"], body: unknown) {
  return app.request("/api/entries", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) });
}
describe("AI registration API", () => {
  test("CLIから準備・登録・再送・未記録分の再確認まで実行できる", async () => {
    const { app, prisma } = setup();
    const dir = fixtures.at(-1)!.dir;
    const codexRoot = path.join(dir, "codex");
    const claudeRoot = path.join(dir, "claude");
    mkdirSync(codexRoot);
    mkdirSync(claudeRoot);
    const file = path.join(codexRoot, "integration.jsonl");
    writeFileSync(file, [
      { type: "session_meta", payload: { id: "integration-session", cwd: process.cwd() } },
      { timestamp: "2026-09-01T10:00:00+09:00" },
      { timestamp: "2026-09-01T10:20:00+09:00" },
    ].map((row) => JSON.stringify(row)).join("\n"));
    writeFileSync(path.join(claudeRoot, "claude-session.jsonl"), [
      { timestamp: "2026-09-01T09:55:00+09:00", sessionId: "claude-session", cwd: "/other-project", type: "user", message: { content: "別案件の調査" } },
      { timestamp: "2026-09-01T10:20:00+09:00", sessionId: "claude-session", type: "assistant", message: { content: "調査完了" } },
    ].map((row) => JSON.stringify(row)).join("\n"));
    const server = Bun.serve({ hostname: "127.0.0.1", port: 0, fetch: app.fetch });
    const cli = async (args: string[]) => {
      const child = Bun.spawn([process.execPath, path.resolve("src/cli/index.ts"), ...args], {
        env: { ...process.env, TRACK_API_BASE: `${server.url}api`, CODEX_THREAD_ID: "integration-session", CODEX_SESSIONS_DIR: codexRoot, TRACK_CLAUDE_PROJECTS_DIR: claudeRoot },
        stdout: "pipe", stderr: "pipe",
      });
      const result = JSON.parse(await new Response(child.stdout).text());
      expect(result).toMatchObject({ ok: true });
      expect(await child.exited).toBe(0);
      return result;
    };
    try {
      const daily = await cli(["prepare", "--date", "2026-09-01"]);
      expect(daily).toMatchObject({ scope: "day", source: "all", roundingMinutes: 15, totalCandidateMinutes: 45 });
      expect(daily.segments).toHaveLength(2);
      expect(daily.sessions.map((session: { source: string }) => session.source).sort()).toEqual(["claude", "codex"]);
      expect(daily.segments.every((segment: { parallelWith: string[] }) => segment.parallelWith.length === 1)).toBe(true);
      const prepared = await cli(["prepare", "--source", "codex", "--session-file", file]);
      expect(prepared.sessionId).toBe("integration-session");
      expect(prepared.segments).toHaveLength(1);
      const segment = prepared.segments[0];
      const args = ["create", "--start", segment.start, "--end", segment.end,
        "--title", "CLI連携", "--source", "codex", "--session-id", prepared.sessionId,
        "--request-id", segment.requestId, "--confirmed"];
      const created = await cli(args);
      expect((await cli(args)).created.id).toBe(created.created.id);
      const next = await cli(["prepare", "--source", "codex", "--session-file", file]);
      expect(next.window).toBeNull();
      expect(next.segments).toEqual([]);
      const nextDay = await cli(["prepare", "--date", "2026-09-01"]);
      expect(nextDay.segments).toHaveLength(1);
      expect(nextDay.segments[0]).toMatchObject({ source: "claude", start: "2026-09-01T09:45:00+09:00", end: "2026-09-01T10:00:00+09:00" });
      expect(nextDay.totalCandidateMinutes).toBe(15);
      expect(await prisma.timeEntry.count()).toBe(1);
    } finally { server.stop(true); }
  });
  test("AI登録は開始・終了を15分単位に制限する", async () => {
    const { app, prisma } = setup();
    const invalid = await post(app, { ...payload(), start: "2026-10-01T10:01:00+09:00" });
    expect(invalid.status).toBe(400);
    expect((await invalid.json()).error).toBe("invalid_time_step");
    expect(await prisma.timeEntry.count()).toBe(0);
  });
  test("同じ要求を再送すると元の工数を返し、履歴を一度だけ保存する", async () => {
    const { app, prisma } = setup();
    const body = payload();
    const first = await post(app, body);
    expect(first.status).toBe(201);
    const created = await first.json();
    const retry = await post(app, body);
    expect(retry.status).toBe(200);
    expect((await retry.json()).id).toBe(created.id);
    expect(await prisma.timeEntry.count()).toBe(1);
    expect(await prisma.aiRegistration.count()).toBe(1);
    const progress = await app.request("/api/entries/ai-progress?source=codex&sessionId=session-a");
    expect(await progress.json()).toEqual({ recordedUntil: "2026-10-01T01:30:00.000Z" });
  });
  test("同じ要求IDで内容を変えた登録を拒否する", async () => {
    const { app, prisma } = setup();
    const body = payload();
    expect((await post(app, body)).status).toBe(201);
    const changed = await post(app, { ...body, title: "別作業" });
    expect(changed.status).toBe(409);
    expect((await changed.json()).error).toBe("request_id_conflict");
    expect(await prisma.timeEntry.count()).toBe(1);
  });
  test("削除済みの工数を再送で復活させない", async () => {
    const { app, prisma } = setup();
    const body = payload();
    const created = await (await post(app, body)).json();
    expect((await app.request(`/api/entries/${created.id}`, { method: "DELETE" })).status).toBe(200);
    const retry = await post(app, body);
    expect(retry.status).toBe(409);
    expect((await retry.json()).error).toBe("registered_entry_deleted");
    expect(await prisma.timeEntry.count()).toBe(0);
    expect(await prisma.aiRegistration.count()).toBe(1);
    const coverage = await app.request("/api/entries/ai-progress?source=codex&sessionId=session-a&coverage=1");
    expect((await coverage.json()).registrations).toEqual([{ recordedFrom: "2026-10-01T01:00:00.000Z", recordedUntil: "2026-10-01T01:30:00.000Z" }]);
  });
  test("重複拒否・DBエラー時は登録済み時刻を進めない", async () => {
    const { app, prisma } = setup();
    const body = payload();
    expect((await post(app, { ...body, aiRegistration: undefined })).status).toBe(201);
    const rejected = await post(app, body);
    expect(rejected.status).toBe(409);
    expect((await rejected.json()).error).toBe("exact_duplicate");
    expect(await prisma.aiRegistration.count()).toBe(0);
    const failure = await post(app, { ...body, start: "2026-10-01T11:00:00+09:00", end: "2026-10-01T11:30:00+09:00", projectId: "missing-project" });
    expect(failure.status).toBe(500);
    expect(await prisma.aiRegistration.count()).toBe(0);
    expect(await prisma.timeEntry.count()).toBe(1);
  });
  test("確認した重複だけを登録し、別セッションの進捗を混ぜない", async () => {
    const { app, prisma } = setup();
    const body = payload();
    expect((await post(app, body)).status).toBe(201);
    const second = { ...body, title: "別作業", aiRegistration: { ...body.aiRegistration, requestId: randomUUID(), sessionId: "session-b" } };
    expect((await post(app, second)).status).toBe(409);
    expect((await post(app, { ...second, aiRegistration: { ...second.aiRegistration, allowOverlap: true } })).status).toBe(201);
    expect(await prisma.aiRegistration.count()).toBe(2);
    const unknown = await app.request("/api/entries/ai-progress?source=claude&sessionId=session-a");
    expect(await unknown.json()).toEqual({ recordedUntil: null });
  });
  test("並行する同じ要求でも工数を一度だけ作成する", async () => {
    const { app, prisma } = setup();
    const body = payload();
    const responses = await Promise.all([post(app, body), post(app, body)]);
    expect(responses.map((r) => r.status).sort()).toEqual([200, 201]);
    const results = await Promise.all(responses.map((r) => r.json()));
    expect(results[0].id).toBe(results[1].id);
    expect(await prisma.timeEntry.count()).toBe(1);
    expect(await prisma.aiRegistration.count()).toBe(1);
  });
  test("日付ごとの登録を再送し、最後の登録時刻を保持する", async () => {
    const { app, prisma } = setup();
    const first = { ...payload(), start: "2026-10-01T23:45:00+09:00", end: "2026-10-02T00:00:00+09:00" };
    const second = { ...payload(), start: "2026-10-02T00:00:00+09:00", end: "2026-10-02T00:30:00+09:00" };
    expect((await post(app, first)).status).toBe(201);
    expect((await post(app, second)).status).toBe(201);
    expect((await post(app, first)).status).toBe(200);
    expect(await prisma.timeEntry.count()).toBe(2);
    expect(await (await app.request("/api/entries/ai-progress?source=codex&sessionId=session-a")).json()).toEqual({ recordedUntil: "2026-10-01T15:30:00.000Z" });
  });
});
